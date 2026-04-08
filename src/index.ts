#!/usr/bin/env node
import { Command } from "commander";
import { registerRunCommand } from "./commands/run.js";
import { registerBeeCommand } from "./commands/bee-cmd.js";
import { registerResultsCommand } from "./commands/results.js";
import { registerJobsCommand } from "./commands/jobs.js";
import { registerSetupCommand } from "./commands/setup.js";
import { registerConfigCommand } from "./commands/config-cmd.js";
import { registerUpdateCommand, checkForUpdateNotice } from "./commands/update.js";

// Silent background update check — prints a one-liner if a newer version exists
checkForUpdateNotice().catch(() => {});

const program = new Command();

program
  .name("beehive")
  .description("Local-first parallel AI agent swarm runner")
  .version("0.5.0");

registerRunCommand(program);
registerBeeCommand(program);
registerResultsCommand(program);
registerJobsCommand(program);
registerSetupCommand(program);
registerConfigCommand(program);
registerUpdateCommand(program);

program.parse(process.argv);
