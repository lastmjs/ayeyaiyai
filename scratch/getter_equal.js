var obj = {
  get value() {
    return "x";
  }
};

console.log("getter-eq", obj.value === "x", obj.value === undefined);
