// husako module: default export with build()

function build(input) {
  let items;
  if (Array.isArray(input)) {
    items = input;
  } else {
    items = [input];
  }

  const rendered = items.map(function(item, index) {
    if (item && typeof item._render === "function") {
      return item._render();
    }
    throw new TypeError(
      "build(): item at index " + index + " is not a builder instance. " +
      "Use builder functions like deployment(), service(), namespace()."
    );
  });

  __husako_build(rendered);
}

const husako = { build };
export default husako;
