import "@testing-library/jest-dom";
import { JSDOM } from "jsdom";

if (typeof window === "undefined" || !globalThis.document) {
  const dom = new JSDOM("<!doctype html><html><body></body></html>", {
    url: "http://localhost",
  });
  const jsdomWindow = dom.window as unknown as Window & typeof globalThis;
  globalThis.window = jsdomWindow;
  globalThis.document = jsdomWindow.document;
  globalThis.navigator = jsdomWindow.navigator;
  globalThis.HTMLElement = jsdomWindow.HTMLElement;
  globalThis.getComputedStyle = jsdomWindow.getComputedStyle;
}
