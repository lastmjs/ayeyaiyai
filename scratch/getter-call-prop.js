var obj = {
  make() {
    var nextCount = 0;
    return {
      get next() {
        return function(value) {
          nextCount++;
          return { count: nextCount, value };
        };
      }
    };
  }
};

var iter = obj.make();
var n = iter.next;
console.log("typeof call", typeof n.call);
