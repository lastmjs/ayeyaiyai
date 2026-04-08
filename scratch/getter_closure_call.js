var nextCount = 0;
var iter = {
  get next() {
    return function() {
      nextCount += 1;
      return {
        get value() {
          return "x";
        },
        get done() {
          return false;
        }
      };
    };
  }
};

var next = iter.next;
var step = next.call(iter, "ignored");
console.log("closure-call", step.value === "x", step.done === false, nextCount);
