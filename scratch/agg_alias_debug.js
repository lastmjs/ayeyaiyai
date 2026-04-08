var obj = {
  [Symbol.iterator]() {
    return {
      get next() {
        return function() {
          return {
            get value() {
              return "next-value-1";
            },
            get done() {
              return false;
            }
          };
        };
      }
    };
  }
};

var callCount = 0;
class C {
  static async *gen() {
    callCount += 1;
    yield* obj;
  }
}

var gen = C.gen;
var iter = gen();
iter.next("x").then(function(v) {
  console.log(
    "debug",
    typeof v.value,
    typeof v.done,
    v.value === undefined,
    v.done === undefined,
    v.value === v,
    v.done === v,
    callCount
  );
});
console.log("after", callCount);
