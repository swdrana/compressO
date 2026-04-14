# CompressO - ইনস্টলেশন ও বিল্ড গাইড

এই গাইডটি আপনাকে সাহায্য করবে কীভাবে আপনার ম্যাক (Mac), উইন্ডোজ (Windows) বা লিনাক্স (Linux)-এ CompressO অ্যাপটির ফাইনাল প্রোডাকশন ভার্সন (.dmg, .exe) তৈরি বা ইনস্টল করবেন।

## পূর্বশর্ত (Prerequisites)
আপনার কম্পিউটারে নিচের জিনিসগুলো ইনস্টল করা থাকতে হবে:

১. **Node.js:** [nodejs.org](https://nodejs.org/) থেকে ইনস্টল করুন।
২. **pnpm:** Node.js ইনস্টল করার পর টার্মিনালে `npm install -g pnpm` রান করে `pnpm` ইনস্টল করুন।
৩. **Rust:** [rustup.rs](https://rustup.rs/) থেকে Rust ইনস্টল করুন। ম্যাকের জন্য টার্মিনালে নিচের কমান্ডটি দিতে পারেন:
   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   ```
৪. **Tauri Prerequisites:** 
   - **Mac:** Xcode Command Line Tools লাগবে (`xcode-select --install`)
   - **Windows:** Visual Studio C++ Build Tools লাগবে
   - **Linux:** `build-essential`, `libwebkit2gtk-4.1-dev` ইত্যাদি প্যাকেজ লাগবে

---

## বিল্ড করার ধাপসমূহ (How to Build)

টার্মিনাল (বা Command Prompt) ওপেন করুন এবং প্রজেক্টের মেইন ফোল্ডারে (যেখানে `package.json` আছে) যান:

### ধাপ ১: ডিপেন্ডেন্সি ইনস্টল করা
প্রথমে নিচের কমান্ডটি দিয়ে সব নেসেসারি প্যাকেজ ইনস্টল করে নিন:
```bash
pnpm install
```

### ধাপ ২: ডেভেলপমেন্ট মোডে টেস্ট করা (ঐচ্ছিক)
অ্যাপটি ঠিকমতো কাজ করছে কিনা তা দেখতে চাইলে নিচের কমান্ডটি রান করতে পারেন:
```bash
pnpm tauri dev
# অথবা
pnpm tauri:dev
```
এটি অ্যাপটির একটি ডেভেলপমেন্ট ভার্সন চালু করবে।

### ধাপ ৩: ফাইনাল অ্যাপ তৈরি করা (Production Build)
আপনার ম্যাক বা পিসিতে ইনস্টল করার জন্য ফাইনাল সেটআপ ফাইল তৈরি করতে নিচের কমান্ডটি দিন:
```bash
pnpm tauri build
# অথবা
pnpm tauri:build
```

**এই কমান্ডটি শেষ হতে কিছুটা সময় লাগবে, কারণ এটি পুরো Rust ব্যাকএন্ড এবং রিঅ্যাক্ট ফ্রন্টএন্ড একসাথে বিল্ড করবে।**

---

## ফাইল কোথায় পাবেন? (Where to find the App)

বিল্ড পুরোপুরি সফল হলে আপনার ইনস্টলেশন ফাইলগুলো নিচের ফোল্ডারগুলোতে পাওয়া যাবে:

*   **Mac এর জন্য (.dmg বা .app):** 
    আপনার প্রজেক্টের ফোল্ডারে যান: `src-tauri/target/release/bundle/macos/` বা `src-tauri/target/release/bundle/dmg/`
    এখানে আপনি `CompressO.dmg` বা `CompressO.app` ফাইল পাবেন। এটিতে ডাবল ক্লিক করলেই সাধারণ ম্যাক অ্যাপের মতো ইনস্টল হয়ে যাবে।

*   **Windows এর জন্য (.msi বা .exe):** 
    `src-tauri\target\release\bundle\msi\` ফোল্ডারে `.msi` ফাইল পাবেন।

*   **Linux এর জন্য (.deb বা .AppImage):** 
    `src-tauri/target/release/bundle/` ফোল্ডারের ভেতরে পাবেন।

---

**অতিরিক্ত তথ্য:**
যেহেতু আমরা `AGENTS.md` ফাইলে দেখেছি আপনার প্রজেক্টটি `pnpm` এবং `Tauri 2.0` ব্যবহার করে, তাই উপরের কমান্ডগুলো আপনার বর্তমান প্রজেক্ট স্ট্রাকচারের জন্য একদম সঠিক।
