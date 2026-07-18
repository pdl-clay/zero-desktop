import { createI18n } from "vue-i18n";
import messages from "@/i18n";

export const LOCALE_STORAGE_KEY = "zero-desktop-locale";

function initialLocale() {
  try {
    const saved = localStorage.getItem(LOCALE_STORAGE_KEY);
    if (saved && messages[saved]) return saved;
  } catch {
    // localStorage unavailable - fall back to the default below.
  }
  return "pt-BR";
}

export const i18n = createI18n({
  locale: initialLocale(),
  globalInjection: true,
  messages,
});
