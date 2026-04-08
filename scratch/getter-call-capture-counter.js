var obj = {
  make() {
    var nextCount = 0;
    return {
      get next() {
        return function() {
          nextCount++;
          return nextCount;
        };
      }
    };
  }
};

var iter = obj.make();
var n = iter.next;
console.log("c1", n.call(iter));
console.log("c2", n.call(iter));
