import {
  Card as NextUICard,
  CardFooter as NextUICardFooter,
  type CardFooterProps as NextUICardFooterProps,
  type CardProps as NextUICardProps,
} from '@heroui/card'

interface CardProps extends NextUICardProps {}

function Card(props: CardProps) {
  return <NextUICard {...props} />
}

interface CardFooterProps extends NextUICardFooterProps {}

export function CardFooter(props: CardFooterProps) {
  return <NextUICardFooter {...props} />
}

export default Card
