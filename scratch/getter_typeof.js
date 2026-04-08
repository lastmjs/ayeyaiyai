var obj = {
  get value() {
    return "x";
  },
  get done() {
    return false;
  }
};

console.log("getter-typeof", typeof obj.value, typeof obj.done);
