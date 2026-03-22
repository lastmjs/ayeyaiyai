function sumTo(limit) {
  let total = 0;

  for (let i = 0; i <= limit; i++) {
    if (i === 2) {
      continue;
    }

    if (i === 5) {
      break;
    }

    total += i;
  }

  return total;
}

let label = false ? "bad" : "good";
console.log(label, sumTo(10), undefined ?? "fallback");
