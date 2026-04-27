/**
 * @license
 * Copyright 2017 Google Inc.
 * SPDX-License-Identifier: Apache-2.0
 */
import { type KeyInput } from '@midscene/shared/us-keyboard-layout';
/**
 * @public
 */
export interface KeyDownOptions {
    /**
     * @deprecated Do not use. This is automatically handled.
     */
    text?: string;
    /**
     * @deprecated Do not use. This is automatically handled.
     */
    commands?: string[];
}
/**
 * @public
 */
export interface KeyboardTypeOptions {
    /**
     * Time to wait between key presses in milliseconds
     * @default undefined
     */
    delay?: number;
}
/**
 * @public
 */
export type KeyPressOptions = KeyDownOptions & KeyboardTypeOptions;
type InternalCDPSession = {
    send: (command: string, params: any) => Promise<void>;
};
/**
 * @internal
 */
export declare class CdpKeyboard {
    #private;
    _modifiers: number;
    constructor(client: InternalCDPSession);
    updateClient(client: InternalCDPSession): void;
    down(key: KeyInput, options?: Readonly<KeyDownOptions>): Promise<void>;
    up(key: KeyInput): Promise<void>;
    sendCharacter(char: string): Promise<void>;
    private charIsKey;
    type(text: string, options?: Readonly<KeyboardTypeOptions>): Promise<void>;
    press(key: KeyInput | KeyInput[], options?: Readonly<KeyPressOptions>): Promise<void>;
}
export {};
