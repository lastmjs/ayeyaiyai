let values = [1, 2];
values[2] = 4;
values[1] += 3;

let person = {
  name: "Aye",
  score: values[1],
};

person.name = "Yai";
console.log(person.name, person.score, values.length, values[2]);
