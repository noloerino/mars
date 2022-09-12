import { init as duna_init, SimState, SimSnapshot } from "duna_web";
import { memory } from "duna_web/duna_web_bg.wasm";
import { CodeMirrorWrapper } from "./components/codemirror";
import { initLayout } from "./components/golden.jsx";

duna_init();

// DOM components must be laid out before codemirror is initialized
initLayout();

let cm = new CodeMirrorWrapper();

let simState = SimState.new();

console.log("You're running the duna simulator, version", BUILD_VERSION);

const assembleProgram = () => {
  const compileErrors = <HTMLPreElement>(
    document.getElementById("compile-errors")
  );
  const program = cm.getProgram();
  simState.assemble(program);
  let errs = simState.get_errors();
  compileErrors.innerText = errs ?? "No errors!";
  if (!errs) {
    updateState();
  }
};

const updateState = () => {
  const stdout = <HTMLTextAreaElement>document.getElementById("stdout");
  const exitCode = <HTMLSpanElement>document.getElementById("exit-code");
  let ec = simState.result();
  exitCode.innerText = ec?.toString() ?? "--";
  let stdoutText = simState.stdout();
  stdout.value = stdoutText ?? "";
  let snapshot = simState.snapshot();
  const state = <HTMLTextAreaElement>document.getElementById("sim-state");
  state.value = snapshot
    ? `PC: ${snapshot.curr_pc()}\n` +
      `next instruction: ${snapshot.curr_inst()}\n\n` +
      `${snapshot.reg_dump()}`
    : "";
};

const step = () => {
  simState.step();
  updateState();
};

const run = () => {
  simState.run();
  updateState();
};

const reset = () => {
  simState.reset();
  updateState();
};

const assembleButton = document.getElementById("assemble");
assembleButton.onclick = (e) => assembleProgram();

const stepButton = document.getElementById("step");
stepButton.onclick = (e) => step();

const runButton = document.getElementById("run");
runButton.onclick = (e) => run();

const resetButton = document.getElementById("reset");
resetButton.onclick = (e) => reset();

window.onbeforeunload = () => {
  cm.save();
};
