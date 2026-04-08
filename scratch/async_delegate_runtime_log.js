var log = [];
var obj = {
  get [Symbol.iterator]() {
    log.push("get-iter");
    return function() {
      log.push("call-iter");
      return {
        get next() {
          log.push("get-next");
          return function() {
            log.push("call-next");
            return {
              get value() {
                log.push("get-value");
                return "next-value-1";
              },
              get done() {
                log.push("get-done");
                return false;
              }
            };
          };
        }
      };
    };
  },
  get [Symbol.asyncIterator]() {
    log.push("get-async");
    return null;
  }
};

class C {
  static async *gen() {
    yield* obj;
  }
}

var iter = C.gen();
iter.next().then(function(v) {
  console.log("value", v.value, "done", v.done);
  console.log("log", log.join("|"));
});
