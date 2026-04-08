var obj = {
  make() {
    var nextCount = 0;
    return {
      name: "syncIterator",
      get next() {
        return function(v) {
          nextCount++;
          return { count: nextCount, thisName: this && this.name, arg: v };
        };
      }
    };
  }
};

var iter = obj.make();
var n = iter.next;
var r1 = n.call(iter, "a");
var r2 = n.call(iter, "b");
console.log("r1", r1.count, r1.thisName, r1.arg);
console.log("r2", r2.count, r2.thisName, r2.arg);
