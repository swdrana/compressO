use crate::domain::{
    BatchImageCompressionProgress, CancelInProgressCompressionPayload, CustomEvents,
    ImageBatchCompressionResult, ImageBatchIndividualCompressionResult, ImageCompressionConfig,
    ImageCompressionProgress, ImageCompressionResult, TauriEvents,
};
use crate::ffmpeg::FFMPEG;
use crate::fs::get_file_metadata;
use image::ImageReader;
use img_parts::{
    jpeg::Jpeg,
    png::{Png, PngChunk},
    Bytes,
};
use log::error;
use oxipng::{optimize, Deflaters, InFile, Options, OutFile, StripChunks};
use resvg::{self, tiny_skia, usvg};
use shared_child::SharedChild;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;
use std::{
    process::{Command, Stdio},
    sync::Arc,
};
use strum::EnumProperty;
use tauri::{AppHandle, Emitter, Listener, Manager};
use tauri_plugin_shell::ShellExt;

#[derive(Debug, Clone, Copy)]
pub enum ImageContainer {
    Png,
    Jpeg,
}

pub const EXTENSIONS: [&str; 6] = ["png", "jpg", "jpeg", "webp", "gif", "svg"];

pub struct ImageCompressor {
    app: AppHandle,
    assets_dir: PathBuf,
}

impl ImageCompressor {
    pub fn new(app: &tauri::AppHandle) -> Result<Self, String> {
        let app_data_dir = match app.path().app_data_dir() {
            Ok(path_buf) => path_buf,
            Err(_) => {
                return Err(String::from(
                    "Application app directory is not setup correctly.",
                ));
            }
        };
        let assets_dir: PathBuf = [PathBuf::from(&app_data_dir), PathBuf::from("assets")]
            .iter()
            .collect();

        Ok(Self {
            app: app.to_owned(),
            assets_dir,
        })
    }

    pub fn get_asset_dir(&self) -> String {
        self.assets_dir.display().to_string()
    }

    pub fn get_pngquant_command(&self) -> Result<Command, String> {
        self.app
            .shell()
            .sidecar("compresso_pngquant")
            .map(Command::from)
            .map_err(|e| format!("Failed to create pngquant command: {}", e))
    }

    pub fn get_jpegoptim_command(&self) -> Result<Command, String> {
        self.app
            .shell()
            .sidecar("compresso_jpegoptim")
            .map(Command::from)
            .map_err(|e| format!("Failed to create jpegoptim command: {}", e))
    }

    pub fn get_gifski_command(&self) -> Result<Command, String> {
        self.app
            .shell()
            .sidecar("compresso_gifski")
            .map(Command::from)
            .map_err(|e| format!("Failed to create gifski command: {}", e))
    }

    pub async fn compress_image(
        &mut self,
        input_path: &str,
        convert_to_extension: Option<&str>,
        is_lossless: Option<bool>,
        quality: u8,
        image_id: &str,
        _batch_id: Option<&str>,
        strip_metadata: Option<bool>,
    ) -> Result<ImageCompressionResult, String> {
        let original_path = Path::new(input_path);
        if !original_path.exists() {
            return Err(String::from("Image file does not exist."));
        }

        let original_metadata = get_file_metadata(input_path)?;

        let original_extension = original_metadata.extension.to_lowercase();
        let output_extension = convert_to_extension.unwrap_or(&original_extension);

        let supported = EXTENSIONS.iter().any(|&ext| ext == output_extension);
        if !supported {
            return Err(format!(
                "Unsupported convert to extension: {}",
                output_extension
            ));
        }

        let output_filename = format!("{}.{}", image_id, output_extension);
        let output_path: PathBuf = [self.assets_dir.clone(), PathBuf::from(&output_filename)]
            .iter()
            .collect();

        let need_conversion =
            convert_to_extension.is_some() && convert_to_extension.unwrap() != &original_extension;

        let intermediate_path: Option<PathBuf> = if need_conversion {
            let temp_filename = format!("{}_temp.{}", image_id, original_extension);
            Some(
                [self.assets_dir.clone(), PathBuf::from(&temp_filename)]
                    .iter()
                    .collect(),
            )
        } else {
            None
        };

        let compression_output_path = intermediate_path.as_ref().unwrap_or(&output_path);

        match original_extension.as_str() {
            "png" => {
                self.compress_png(
                    input_path,
                    compression_output_path.to_str().unwrap(),
                    is_lossless.unwrap_or(true),
                    quality,
                    strip_metadata.unwrap_or(true),
                )
                .await?
            }
            "jpg" | "jpeg" => {
                self.compress_jpeg(
                    input_path,
                    compression_output_path.to_str().unwrap(),
                    is_lossless.unwrap_or(true),
                    quality,
                    strip_metadata.unwrap_or(true),
                )
                .await?
            }
            "webp" => {
                self.compress_webp(
                    input_path,
                    compression_output_path.to_str().unwrap(),
                    is_lossless.unwrap_or(true),
                    quality,
                    strip_metadata.unwrap_or(true),
                )
                .await?
            }
            "gif" => {
                self.compress_gif(
                    input_path,
                    compression_output_path.to_str().unwrap(),
                    is_lossless.unwrap_or(true),
                    quality,
                    strip_metadata.unwrap_or(true),
                )
                .await?
            }
            "svg" => {
                self.compress_svg(
                    input_path,
                    compression_output_path.to_str().unwrap(),
                    is_lossless.unwrap_or(true),
                    quality,
                    strip_metadata.unwrap_or(true),
                )
                .await?
            }
            _ => {
                return Err(format!(
                    "Unsupported source format: {}. Original file will be copied.",
                    original_extension
                ))
            }
        };

        tokio::time::sleep(Duration::from_secs(3)).await;

        if need_conversion {
            let convert_to_ext = convert_to_extension.unwrap();
            match original_extension.as_str() {
                "png" | "jpg" | "jpeg" | "webp" => {
                    if convert_to_ext.eq("svg") {
                        let svg_result = self.convert_raster_img_to_svg(
                            compression_output_path.to_str().unwrap(),
                            output_path.to_str().unwrap(),
                            quality,
                        );

                        // If above conversion fails, retry with png
                        if svg_result.is_err() && original_extension != "png" {
                            log::warn!("[image] Direct SVG conversion failed, converting to PNG and retrying");

                            let temp_png_path = format!(
                                "{}_temp_png.png",
                                output_path.to_str().unwrap().trim_end_matches(".svg")
                            );

                            match self
                                .convert_image_via_ffmpeg(
                                    compression_output_path.to_str().unwrap(),
                                    &temp_png_path,
                                    "png",
                                    100, // full quality
                                    true,
                                )
                                .await
                            {
                                Ok(_) => {
                                    let retry_result = self.convert_raster_img_to_svg(
                                        &temp_png_path,
                                        output_path.to_str().unwrap(),
                                        quality,
                                    );

                                    // Clean up temp png file
                                    let _ = fs::remove_file(&temp_png_path);

                                    retry_result?;
                                }
                                Err(e) => {
                                    log::error!("[image] PNG conversion for retry failed: {}", e);
                                    return Err(format!("SVG conversion failed: {}", e));
                                }
                            }
                        } else {
                            svg_result?;
                        }
                    } else {
                        self.convert_image_via_ffmpeg(
                            compression_output_path.to_str().unwrap(),
                            output_path.to_str().unwrap(),
                            output_extension,
                            quality,
                            strip_metadata.unwrap_or(true),
                        )
                        .await?;
                    }
                }
                "svg" => {
                    self.convert_svg_to_png(compression_output_path.to_str().unwrap(), image_id)?
                }
                _ => return Err("Unsupported convert_to_extension".to_string()),
            }

            // Clean up intermediate temp file
            if let Some(intermediate) = intermediate_path {
                let _ = fs::remove_file(&intermediate);
            }
        }

        let compressed_metadata = get_file_metadata(&output_path.to_string_lossy().to_string())?;

        Ok(ImageCompressionResult {
            image_id: image_id.to_string(),
            file_name: output_filename,
            file_path: output_path.display().to_string(),
            file_metadata: Some(compressed_metadata),
        })
    }

    async fn compress_png(
        &mut self,
        input_path: &str,
        output_path: &str,
        is_lossless: bool,
        quality: u8,
        strip_metadata: bool,
    ) -> Result<(), String> {
        if is_lossless {
            let mut options = Options::default();

            options.deflate = Deflaters::Libdeflater { compression: 12 };
            options.optimize_alpha = true;

            options.strip = if strip_metadata {
                StripChunks::All
            } else {
                StripChunks::Safe
            };

            optimize(
                &InFile::Path(PathBuf::from(input_path)),
                &OutFile::Path {
                    path: Some(PathBuf::from(output_path)),
                    preserve_attrs: !strip_metadata,
                },
                &options,
            )
            .map_err(|e| format!("PNG optimization failed: {:?}", e))?;

            if !strip_metadata {
                let _ = self.copy_image_metadata(ImageContainer::Png, input_path, output_path);
            }

            return Ok(());
        } else {
            std::fs::copy(input_path, &output_path).map_err(|e| e.to_string())?;

            let quality_str = quality.clamp(1, 100).to_string();

            let result = self
                .run_pngquant(&quality_str, output_path, input_path, strip_metadata)
                .await;

            match result {
                Err(e) => {
                    log::warn!(
                        "[image] pngquant attempt 1 failed: {}. Retrying with quality 0-{}...",
                        e,
                        quality
                    );

                    self.run_pngquant(
                        &format!("0-{}", quality),
                        output_path,
                        input_path,
                        strip_metadata,
                    )
                    .await?;
                    return Ok(());
                }
                Ok(_) => {
                    return Ok(());
                }
            }
        }
    }

    async fn run_pngquant(
        &mut self,
        quality_str: &str,
        output_path: &str,
        input_path: &str,
        strip_metadata: bool,
    ) -> Result<PathBuf, String> {
        let mut args: Vec<&str> = vec!["--quality", quality_str, "--force", "--strip"];

        args.push("--output");
        args.push(output_path);
        args.push(input_path);

        log::info!("[image] pngquant command: {:?}", args);

        let mut pngquant_cmd = self.get_pngquant_command()?;

        let command = pngquant_cmd
            .args(&args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        match SharedChild::spawn(command) {
            Ok(child) => {
                #[cfg(debug_assertions)]
                let stderr_output = child.take_stderr().and_then(|mut stderr| {
                    use std::io::Read;
                    let mut buffer = String::new();
                    let _ = stderr.read_to_string(&mut buffer);
                    Some(buffer)
                });
                #[cfg(not(debug_assertions))]
                let stderr_output = None;

                let cp = Arc::new(child);
                let cp_clone = cp.clone();

                tokio::spawn(async move {
                    let _ = cp_clone.wait();
                });

                match cp.wait() {
                    Ok(status) if status.success() => {
                        if !strip_metadata {
                            let _ = self.copy_image_metadata(
                                ImageContainer::Png,
                                input_path,
                                output_path,
                            );
                        }
                        Ok(PathBuf::from(output_path))
                    }
                    Ok(_) => {
                        let error_msg = stderr_output
                            .as_ref()
                            .map(|s: &String| s.trim())
                            .unwrap_or("");
                        if !error_msg.is_empty() {
                            Err(format!("pngquant failed: {}", error_msg))
                        } else {
                            Err(String::from("pngquant failed"))
                        }
                    }
                    Err(e) => Err(format!("pngquant error: {}", e)),
                }
            }
            Err(e) => Err(format!("Failed to run pngquant: {}", e)),
        }
    }

    async fn compress_jpeg(
        &mut self,
        input_path: &str,
        output_path: &str,
        is_lossless: bool,
        quality: u8,
        strip_metadata: bool,
    ) -> Result<(), String> {
        use std::process::Stdio;
        use std::sync::Arc;

        std::fs::copy(input_path, &output_path).map_err(|e| e.to_string())?;

        let jpeg_quality = quality.max(1).min(100).to_string();

        let mut args: Vec<&str> = vec!["-o", "-q", "--all-progressive", "--strip-all"];

        if is_lossless {
            args.push("--max=100");
        } else {
            args.push("-m");
            args.push(&jpeg_quality);
        }

        args.push(output_path);

        log::info!("[image] jpegoptim final command{:?}", args);

        let mut jpegoptim_cmd = self.get_jpegoptim_command()?;

        let command = jpegoptim_cmd
            .args(&args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        match SharedChild::spawn(command) {
            Ok(child) => {
                let cp = Arc::new(child);
                let cp_clone = cp.clone();

                tokio::spawn(async move {
                    let _ = cp_clone.wait();
                });

                match cp.wait() {
                    Ok(status) if status.success() => {
                        if !strip_metadata {
                            let _ = self.copy_image_metadata(
                                ImageContainer::Jpeg,
                                input_path,
                                output_path,
                            );
                        }
                        Ok(())
                    }
                    Ok(_) => Err(String::from("jpegoptim failed")),
                    Err(e) => Err(format!("jpegoptim error: {}", e)),
                }
            }
            Err(e) => Err(format!("Failed to run jpegoptim: {}", e)),
        }
    }

    async fn compress_webp(
        &mut self,
        input_path: &str,
        output_path: &str,
        is_lossless: bool,
        quality: u8,
        strip_metadata: bool,
    ) -> Result<(), String> {
        let img = ImageReader::open(input_path)
            .map_err(|e| e.to_string())?
            .decode()
            .map_err(|e| e.to_string())?;

        let width = img.width();
        let height = img.height();
        let rgba: Vec<u8> = img.to_rgba8().into_raw();

        let webp_data = if is_lossless {
            let encoder = webp::Encoder::from_rgba(&rgba, width, height);
            encoder.encode_lossless()
        } else {
            let encoder_quality = quality.clamp(0, 100);
            let encoder = webp::Encoder::from_rgba(&rgba, width, height);
            encoder.encode(encoder_quality as f32)
        };

        if strip_metadata {
            // No support for webp container for now
        }
        std::fs::write(&output_path, webp_data.to_vec()).map_err(|e| e.to_string())?;

        Ok(())
    }

    async fn compress_gif(
        &mut self,
        input_path: &str,
        output_path: &str,
        is_lossless: bool,
        quality: u8,
        strip_metadata: bool,
    ) -> Result<(), String> {
        let output_path_str = output_path.to_string();

        let gifski_quality = if is_lossless {
            100 // No support for true lossless, we'll just compress into highest quality
        } else {
            quality.clamp(1, 100)
        };

        let quality_str = gifski_quality.to_string();

        let args = vec!["-Q", &quality_str, "-o", &output_path_str, input_path];

        if strip_metadata {
            // No support for gif container for now
        }

        log::info!("[image] gifski command: {:?}", args);

        let mut gifski_cmd = self.get_gifski_command()?;

        let command = gifski_cmd
            .args(&args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let child =
            SharedChild::spawn(command).map_err(|e| format!("Failed to run gifski: {}", e))?;

        let cp = Arc::new(child);

        match cp.wait() {
            Ok(status) if status.success() => {}
            Ok(_) => return Err("gifski failed".into()),
            Err(e) => return Err(format!("gifski error: {}", e)),
        };

        Ok(())
    }

    async fn compress_svg(
        &mut self,
        input_path: &str,
        output_path: &str,
        _is_lossless: bool, // irrelevant
        _quality: u8,       // irrelevant
        strip_metadata: bool,
    ) -> Result<(), String> {
        let svg_content =
            svgcleaner::cleaner::load_file(input_path).map_err(|err| err.to_string())?;

        let tree = usvg::Tree::from_str(&svg_content, &usvg::Options::default()).unwrap();
        let normalized_svg = tree.to_string(&usvg::WriteOptions::default());

        let mut doc =
            svgcleaner::cleaner::parse_data(&normalized_svg, &svgcleaner::ParseOptions::default())
                .map_err(|err| err.to_string())?;

        svgcleaner::cleaner::clean_doc(
            &mut doc,
            &svgcleaner::CleaningOptions {
                remove_unused_defs: true,
                resolve_use: false,
                ungroup_groups: false,
                ungroup_defs: false,
                group_by_style: false,
                merge_gradients: false,
                remove_title: true,
                remove_desc: true,
                remove_metadata: strip_metadata,
                remove_dupl_linear_gradients: true,
                remove_dupl_radial_gradients: true,
                remove_dupl_fe_gaussian_blur: true,
                remove_invalid_stops: true,
                remove_invisible_elements: true,
                remove_version: true,
                remove_unreferenced_ids: true,
                trim_ids: true,
                remove_xmlns_xlink_attribute: true,
                remove_needless_attributes: true,
                remove_gradient_attributes: true,
                remove_default_attributes: true,
                join_style_attributes: svgcleaner::StyleJoinMode::All,
                apply_transform_to_shapes: true,
                apply_transform_to_paths: true,
                apply_transform_to_gradients: true,
                paths_to_relative: true,
                convert_shapes: false,
                convert_segments: false,
                remove_unused_segments: false,
                coordinates_precision: 2,
                properties_precision: 2,
                paths_coordinates_precision: 2,
                transforms_precision: 2,
                remove_text_attributes: false,
                remove_unused_coordinates: false,
                regroup_gradient_stops: false,
            },
            &svgcleaner::WriteOptions::default(),
        )
        .map_err(|err| err.to_string())?;

        svgcleaner::cleaner::save_file(&doc.to_string().into_bytes(), output_path)
            .map_err(|err| err.to_string())?;

        Ok(())
    }

    pub fn convert_raster_img_to_svg(
        &self,
        input_path: &str,
        output_path: &str,
        quality: u8,
    ) -> Result<(), String> {
        let q = quality.clamp(1, 100) as f32;
        let filter_speckle = ((100.0 - q) * (128.0 / 99.0)).round() as usize;

        // @ref: https://www.visioncortex.org/vtracer/
        vtracer::convert_image_to_svg(
            &PathBuf::from(input_path),
            &PathBuf::from(output_path),
            vtracer::Config {
                filter_speckle,
                color_precision: 8,
                layer_difference: 0,
                corner_threshold: 180,
                splice_threshold: 0,
                length_threshold: 8.0,
                ..vtracer::Config::from_preset(vtracer::Preset::Photo)
            },
        )
        .map_err(|err| err.to_string())?;

        Ok(())
    }

    pub fn convert_svg_to_png(&self, input_path: &str, image_id: &str) -> Result<(), String> {
        let output_filename = format!("{}.png", image_id);
        let output_path: PathBuf = [self.assets_dir.clone(), PathBuf::from(&output_filename)]
            .iter()
            .collect();

        let svg_data = fs::read(input_path).map_err(|err| err.to_string())?;
        let svg_string = std::str::from_utf8(&svg_data).map_err(|err| err.to_string())?;

        let tree = usvg::Tree::from_str(svg_string, &usvg::Options::default())
            .map_err(|err| err.to_string())?;

        let pixmap_size = tree.size().to_int_size();

        let mut pixmap = tiny_skia::Pixmap::new(pixmap_size.width(), pixmap_size.height())
            .ok_or("Failed to create pixmap")?;

        resvg::render(&tree, tiny_skia::Transform::default(), &mut pixmap.as_mut());

        pixmap
            .save_png(output_path.clone())
            .map_err(|err| err.to_string())?;

        Ok(())
    }

    pub async fn compress_images_batch(
        &mut self,
        batch_id: &str,
        images: Vec<ImageCompressionConfig>,
    ) -> Result<ImageBatchCompressionResult, String> {
        let mut results: std::collections::HashMap<String, ImageCompressionResult> =
            std::collections::HashMap::new();
        let total_count = images.len();

        for (index, image_config) in images.iter().enumerate() {
            let image_id = &image_config.image_id;

            let app_clone = self.app.clone();
            let batch_id_clone = batch_id.to_string();
            let image_id_clone = image_id.clone();

            tokio::spawn(async move {
                if let Some(window) = app_clone.get_webview_window("main") {
                    let _ = window.clone().listen(
                        CustomEvents::ImageCompressionProgress.as_ref(),
                        move |evt| {
                            if let Ok(progress) =
                                serde_json::from_str::<ImageCompressionProgress>(evt.payload())
                            {
                                if progress.image_id == image_id_clone {
                                    let batch_progress = BatchImageCompressionProgress {
                                        batch_id: batch_id_clone.to_owned(),
                                        current_index: index,
                                        total_count,
                                        image_progress: progress,
                                    };
                                    let _ = window.emit(
                                        CustomEvents::BatchImageCompressionProgress.as_ref(),
                                        batch_progress,
                                    );
                                }
                            }
                        },
                    );
                }
            });

            let input_path = &image_config.image_path;
            let quality = image_config.quality;
            let convert_to_extension = image_config.convert_to_extension.as_deref();
            let strip_metadata = image_config.strip_metadata.unwrap_or(true);
            let is_lossless = image_config.is_lossless;

            match self
                .compress_image(
                    input_path,
                    convert_to_extension,
                    is_lossless,
                    quality,
                    image_id,
                    Some(batch_id),
                    Some(strip_metadata),
                )
                .await
            {
                Ok(result) => {
                    let image_id = result.image_id.clone();
                    results.insert(image_id.clone(), result.clone());

                    let app_clone2 = self.app.clone();
                    let batch_id_clone2 = batch_id.to_string();

                    tokio::spawn(async move {
                        if let Some(window) = app_clone2.get_webview_window("main") {
                            let individual_result = ImageBatchIndividualCompressionResult {
                                batch_id: batch_id_clone2,
                                result,
                            };
                            let _ = window.emit(
                                CustomEvents::BatchImageIndividualCompressionCompletion.as_ref(),
                                individual_result,
                            );
                        }
                    });
                }
                Err(e) => {
                    error!("Failed to compress image at index {}: {}", index, e);
                }
            }
        }

        Ok(ImageBatchCompressionResult { results })
    }

    pub async fn convert_image_via_ffmpeg(
        &self,
        input_path: &str,
        output_path: &str,
        output_format: &str,
        quality: u8,
        strip_metadata: bool,
    ) -> Result<(), String> {
        if !PathBuf::from(input_path).exists() {
            return Err(String::from("Input image file does not exist."));
        }

        let mut cmd_args: Vec<String> = Vec::new();

        cmd_args.push("-i".to_string());
        cmd_args.push(input_path.to_string());

        let output_format_lower = output_format.to_lowercase();
        match output_format_lower.as_str() {
            "jpg" | "jpeg" => {
                let jpeg_qscale = (((100 - quality) as f32 / 99.0) * 9.0 + 1.0).round() as u32;
                cmd_args.push("-q:v".to_string()); // 1-31 scale (lower is better) but we'll interpolate between 1-10 only
                cmd_args.push(jpeg_qscale.to_string());
                cmd_args.push("-filter_complex".to_string());
                cmd_args.push(
                    "color=black:s=1x1[bg];[bg][0:v]scale2ref[bg][img];[bg][img]overlay[out]"
                        .to_string(),
                );
                cmd_args.push("-map".to_string());
                cmd_args.push("[out]".to_string());
                cmd_args.push("-frames:v".to_string());
                cmd_args.push("1".to_string());
            }
            "png" => {
                let compression_level = (((quality as f32 - 1.0) / 99.0 * 4.0) + 5.0).round() as u8;
                cmd_args.push("-compression_level".to_string());
                cmd_args.push(compression_level.to_string()); // 0-9 (higher is better) but we'll interpolate between 4-9 only
                cmd_args.push("-pred".to_string());
                cmd_args.push("mixed".to_string());
            }
            "webp" => {
                let webp_quality = ((quality as f32 - 1.0) / 99.0 * 50.0 + 50.0).round() as u8;
                cmd_args.push("-q:v".to_string());
                cmd_args.push(webp_quality.to_string()); // 0-100 (higher is better) but we'll interpolate between 50-100 only
                cmd_args.push("-lossless".to_string());
                cmd_args.push("0".to_string());
            }
            _ => {
                return Err("Unsupported output format".to_string());
            }
        }

        if strip_metadata {
            cmd_args.push("-map_metadata".to_string());
            cmd_args.push("-1".to_string());
        }

        cmd_args.push("-y".to_string());
        cmd_args.push(output_path.to_string());

        log::info!("[ffmpeg-image] conversion command: {:?}", cmd_args);

        let ffmpeg = FFMPEG::new(&self.app)?;
        let mut ffmpeg_cmd = ffmpeg.get_ffmpeg_command()?;

        let command = ffmpeg_cmd
            .args(&cmd_args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        match SharedChild::spawn(command) {
            Ok(child) => {
                let cp = Arc::new(child);
                let cp_clone1 = cp.clone();
                let cp_clone2 = cp.clone();

                let window = match self.app.get_webview_window("main") {
                    Some(window) => window,
                    None => return Err(String::from("Could not attach to main window")),
                };

                let destroy_event_id =
                    window.listen(TauriEvents::Destroyed.get_str("key").unwrap(), move |_| {
                        log::info!("[tauri] window destroyed");
                        let _ = cp.kill();
                    });

                let cancel_event_id = window.listen(
                    CustomEvents::CancelInProgressCompression.as_ref(),
                    move |evt| {
                        let payload_str = evt.payload();
                        if let Ok(_payload) =
                            serde_json::from_str::<CancelInProgressCompressionPayload>(payload_str)
                        {
                            let _ = cp_clone2.kill();
                        }
                    },
                );

                let handle = tokio::spawn(async move {
                    match cp_clone1.wait() {
                        Ok(status) if status.success() => Ok(()),
                        Ok(_) => Err(String::from("FFmpeg conversion failed")),
                        Err(e) => Err(format!("FFmpeg error: {}", e)),
                    }
                });

                match handle.await {
                    Ok(result) => {
                        window.unlisten(destroy_event_id);
                        window.unlisten(cancel_event_id);

                        result?;

                        if !PathBuf::from(output_path).exists() {
                            return Err(String::from("Output file was not created"));
                        }

                        let output_metadata = get_file_metadata(&output_path)?;
                        if output_metadata.size == 0 {
                            return Err(String::from("Output file is empty"));
                        }

                        Ok(())
                    }
                    Err(e) => Err(format!("Conversion task failed: {}", e)),
                }
            }
            Err(e) => Err(format!("Failed to spawn FFmpeg: {}", e)),
        }
    }

    pub fn copy_image_metadata(
        &self,
        container: ImageContainer,
        src: &str,
        dst: &str,
    ) -> Result<(), String> {
        match container {
            ImageContainer::Png => {
                let src_bytes = fs::read(src).map_err(|err| err.to_string())?;
                let dst_bytes = fs::read(dst).map_err(|err| err.to_string())?;

                let src_png =
                    Png::from_bytes(Bytes::from(src_bytes)).map_err(|err| err.to_string())?;
                let mut dst_png =
                    Png::from_bytes(Bytes::from(dst_bytes)).map_err(|err| err.to_string())?;

                let metadata_kinds = [b"tEXt", b"zTXt", b"iTXt", b"iCCP", b"eXIf"];
                let mut chunks_to_keep: Vec<PngChunk> = Vec::new();

                for chunk in dst_png.chunks() {
                    let kind = chunk.kind();
                    let is_metadata = metadata_kinds.iter().any(|&mk| kind == *mk);
                    if !is_metadata {
                        chunks_to_keep.push(chunk.clone());
                    }
                }

                dst_png.chunks_mut().clear();
                for chunk in chunks_to_keep {
                    dst_png.chunks_mut().push(chunk);
                }

                for chunk in src_png.chunks() {
                    let kind = chunk.kind();
                    if kind == *b"tEXt"
                        || kind == *b"zTXt"
                        || kind == *b"iTXt"
                        || kind == *b"iCCP"
                        || kind == *b"eXIf"
                    {
                        dst_png
                            .chunks_mut()
                            .insert(1, PngChunk::new(kind, chunk.contents().clone()));
                    }
                }

                let encoded_bytes = dst_png.encoder().bytes();
                fs::write(dst, encoded_bytes).map_err(|err| err.to_string())?;
            }
            ImageContainer::Jpeg => {
                let src_bytes = fs::read(src).map_err(|err| err.to_string())?;
                let dst_bytes = fs::read(dst).map_err(|err| err.to_string())?;

                let src_jpeg =
                    Jpeg::from_bytes(Bytes::from(src_bytes)).map_err(|err| err.to_string())?;
                let mut dst_jpeg =
                    Jpeg::from_bytes(Bytes::from(dst_bytes)).map_err(|err| err.to_string())?;

                for segment in src_jpeg.segments() {
                    let marker = segment.marker();
                    if (0xE0..=0xEF).contains(&marker) || marker == 0xFE {
                        dst_jpeg.segments_mut().insert(1, segment.clone());
                    }
                }

                fs::write(dst, dst_jpeg.encoder().bytes()).map_err(|err| err.to_string())?;
            }
        }

        Ok(())
    }
}
