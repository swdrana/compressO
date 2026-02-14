import {
  Select as NextUISelect,
  SelectItem as NextUISelectItem,
  type SelectItemProps as NextUISelectItemProps,
  type SelectProps as NextUISelectProps,
} from '@heroui/react'

interface SelectProps extends NextUISelectProps {}
function Select(props: SelectProps) {
  return (
    <NextUISelect radius="md" size="sm" labelPlacement="outside" {...props} />
  )
}

interface SelectItemProps extends NextUISelectItemProps {}
export function SelectItem(props: SelectItemProps) {
  return <NextUISelectItem {...props} />
}

export default Select
