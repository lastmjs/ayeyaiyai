var obj = {
  make() {
    var nextCount = 0;
    return {
      get next() {
        return function(v) {
          nextCount++;
          return { count: nextCount, arg: v };
        };
      }
    };
  }
};

var iter = obj.make();
var n = iter.next;
var r1 = n("a");
var r2 = n("b");
console.log("r1", r1.count, r1.arg);
console.log("r2", r2.count, r2.arg);
