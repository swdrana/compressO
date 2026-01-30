import { Variants } from 'framer-motion'

export const zoomInTransition: Variants = {
  initial: { scale: 0.9, opacity: 0 },
  animate: {
    scale: 1,
    opacity: 1,
    transition: {
      duration: 0.6,
      bounce: 0.3,
      type: 'spring',
    },
  },
  exit: {
    scale: 0.9,
    opacity: 0,
    transition: {
      duration: 0.2,
      bounce: 0.2,
      type: 'spring',
    },
  },
} as const

export const slideDownTransition: Variants = {
  initial: { y: -10, opacity: 0 },
  animate: {
    y: 0,
    opacity: 1,
    transition: {
      duration: 0.6,
      bounce: 0.3,
      type: 'spring',
    },
  },
  exit: {
    y: -10,
    opacity: 0,
    transition: {
      duration: 0.2,
      bounce: 0.2,
      type: 'spring',
    },
  },
} as const

export const zoomInStaggerAnimation: { container: Variants; item: Variants } = {
  container: {
    hidden: { opacity: 0 },
    show: {
      opacity: 1,
      transition: {
        staggerChildren: 0.08,
      },
    },
  },
  item: {
    hidden: { opacity: 0, scale: 0.9 },
    show: { opacity: 1, scale: 1 },
    exit: { opacity: 0, scale: 0.9 },
  },
}
