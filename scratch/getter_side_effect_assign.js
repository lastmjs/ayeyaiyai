var log = [];
var obj = {
  get value() {
    log.push("hit");
    return "x";
  }
};

var value = obj.value;
console.log("assign", value, log.length);
