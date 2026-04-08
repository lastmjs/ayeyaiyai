var obj = {
  get value() {
    return "x";
  },
  get done() {
    return false;
  }
};

console.log("getter-direct", obj.value, obj.done);
