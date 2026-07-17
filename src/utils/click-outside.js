/**
 * Vue directive: calls binding.value() when a click lands outside el.
 * Shared by ChatInput.vue's model/advisor-settings dropdowns and
 * ModelPickerDropdown.vue so both close the same way without duplicating
 * the listener wiring.
 */
export const vClickOutside = {
  mounted(el, binding) {
    el._clickOutside = (event) => {
      if (!(el === event.target || el.contains(event.target))) {
        binding.value();
      }
    };
    document.addEventListener("click", el._clickOutside, true);
  },
  unmounted(el) {
    document.removeEventListener("click", el._clickOutside, true);
  },
};
