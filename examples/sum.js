let total = 0;
let i = 1;

while (i <= 5) {
  total = total + i;
  i = i + 1;
}

if (total === 15) {
  console.log("sum", total);
} else {
  console.log("unexpected", total);
}
