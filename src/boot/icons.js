import { addCollection } from "@iconify/vue";
import svgSpinners from "@iconify-json/svg-spinners/icons.json";
import lineMd from "@iconify-json/line-md/icons.json";

// Registers icon data locally so <Icon> never hits the network at runtime -
// required for a desktop app that must work fully offline.
addCollection(svgSpinners);
addCollection(lineMd);
