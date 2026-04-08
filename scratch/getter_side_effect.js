var log = [];
var obj = {
  get value() {
    log.push("hit");
    return "x";
  }
};

console.log("side", obj.value, log.length);
