import { createI18n } from "vue-i18n";
import messages from "@/i18n";

export const i18n = createI18n({
  locale: "pt-BR",
  globalInjection: true,
  messages,
});
