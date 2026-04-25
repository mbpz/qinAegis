import type { AbstractInterface } from '../device';
type DeviceClass = new (...args: any[]) => AbstractInterface;
export interface SkillCLIOptions {
    scriptName: string;
    DeviceClass: DeviceClass;
}
/**
 * Launch a Skill CLI for a custom interface Device class.
 * This enables AI coding assistants (Claude Code, Cline, etc.) to control
 * your custom interface through CLI commands.
 *
 * @example
 * ```typescript
 * #!/usr/bin/env node
 * import { runSkillCLI } from '@midscene/core/skill';
 * import { SampleDevice } from './sample-device';
 *
 * runSkillCLI({
 *   DeviceClass: SampleDevice,
 *   scriptName: 'my-device',
 * });
 * ```
 */
export declare function runSkillCLI(options: SkillCLIOptions): Promise<void>;
export {};
