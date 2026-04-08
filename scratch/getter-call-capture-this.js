var obj = {
  make() {
    var nextCount = 0;
    return {
      name: "syncIterator",
      get next() {
        return function() {
          nextCount++;
          return this && this.name;
        };
      }
    };
  }
};

var iter = obj.make();
var n = iter.next;
console.log("t1", n.call(iter));
console.log("t2", n.call(iter));
