var obj = {
  get [Symbol.iterator]() {
    return function() {
      var log = [];
      var nextCount = 0;
      return {
        name: "syncIterator",
        get next() {
          return function() {
            log.push(["next-call", arguments.length, nextCount]);
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
    console.log("after-yield-star", value);
    return value;
  }
}

var iter = C.gen();
iter.next("first").then(first => {
  console.log("first-callback", first.value, first.done, typeof first.value, typeof first.done);
  var secondPromise = iter.next("second");
  console.log("after-second-call", typeof secondPromise, typeof secondPromise.then);
  secondPromise.then(second => {
    console.log("second-callback", second.value, second.done);
    console.log(first.value, second.value);
  });
});
