var log = [];
var obj = {
  get value() {
    log.push("hit");
    return "x";
  }
};

console.log("side-eq", obj.value === "x", obj.value === undefined, log.length);
