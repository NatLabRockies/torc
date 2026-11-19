// Table filter operators (features/details/filter.ts).
import assert from "node:assert/strict";
import { matchesOperator, parseOperatorFilter } from "../src/features/details/filter.ts";

const match = (expr: string, row: object) => matchesOperator(row, parseOperatorFilter(expr)!);

// status: = != and contains (~ :)
assert.ok(match("status:fail", { status: "failed" }));
assert.ok(match("status~fail", { status: "failed" }));
assert.ok(!match("status:fail", { status: "completed" }));
assert.ok(match("status=failed", { status: "Failed" }));
// Numbers compare numerically only when the whole value is a number.
assert.ok(!match("name=1_train", { name: "1_eval" }));
assert.ok(match("name!=1_train", { name: "1_eval" }));
assert.ok(match("name=1_train", { name: "1_train" }));
assert.ok(match("id>5", { id: 10 }));
assert.ok(!match("id>50", { id: 10 }));
assert.ok(match("return_code=0", { return_code: 0 }));
assert.ok(match("exec_time>=1.5", { exec_time_minutes: "2.0" }));
console.log("filter checks passed");
