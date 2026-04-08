var obj = {
  get [Symbol.iterator]() {
    return function() {
      var log = [];
      var nextCount = 0;
      return {
        name: "syncIterator",
        get next() {
          return function() {
            log.push(arguments.length);
            nextCount++;
            if (nextCount === 1) {
              return {
                value: "next-value-1",
                done: false,
              };
            }
            return {
              value: "next-value-2",
              done: true,
            };
          };
        },
      };
    };
  },
  get [Symbol.asyncIterator]() {
    return null;
  },
};

class C {
  static async *gen() {
    var value = yield* obj;
    return value;
  }
}

var iter = C.gen();
iter.next("first").then(first => {
  console.log("first-hit", typeof first.value, first.value, first.done);
  iter.next("second").then(second => {
    console.log("second-hit", typeof first.value, typeof second.value, first.value, second.value);
  });
});
