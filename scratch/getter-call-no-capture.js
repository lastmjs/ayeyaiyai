var obj = {
  make() {
    return {
      get next() {
        return function(value) {
          return value;
        };
      }
    };
  }
};

var iter = obj.make();
var n = iter.next;
console.log("call", n.call(iter, "ok"));
