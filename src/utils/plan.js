export function planIcon(status) {
  switch (status) {
    case "completed":
      return "check_circle";
    case "in_progress":
      return "autorenew";
    case "failed":
      return "cancel";
    default:
      return "radio_button_unchecked";
  }
}

export function planColor(status) {
  switch (status) {
    case "completed":
      return "positive";
    case "in_progress":
      return "info";
    case "failed":
      return "negative";
    default:
      return "grey-6";
  }
}
