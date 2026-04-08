var obj = {
  get value() {
    return "x";
  },
  get done() {
    return false;
  }
};

console.log(
  "getter",
  typeof obj.value,
  typeof obj.done,
  obj.value === undefined,
  obj.done === undefined
);
