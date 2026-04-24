/*! For license information please see cdpInput.mjs.LICENSE.txt */
import { _keyDefinitions } from "@midscene/shared/us-keyboard-layout";
import { assert } from "@midscene/shared/utils";
/**
 * @license
 * Copyright 2017 Google Inc.
 * SPDX-License-Identifier: Apache-2.0
 */ function _check_private_redeclaration(obj, privateCollection) {
    if (privateCollection.has(obj)) throw new TypeError("Cannot initialize the same private elements twice on an object");
}
function _class_apply_descriptor_get(receiver, descriptor) {
    if (descriptor.get) return descriptor.get.call(receiver);
    return descriptor.value;
}
function _class_apply_descriptor_set(receiver, descriptor, value) {
    if (descriptor.set) descriptor.set.call(receiver, value);
    else {
        if (!descriptor.writable) throw new TypeError("attempted to set read only private field");
        descriptor.value = value;
    }
}
function _class_extract_field_descriptor(receiver, privateMap, action) {
    if (!privateMap.has(receiver)) throw new TypeError("attempted to " + action + " private field on non-instance");
    return privateMap.get(receiver);
}
function _class_private_field_get(receiver, privateMap) {
    var descriptor = _class_extract_field_descriptor(receiver, privateMap, "get");
    return _class_apply_descriptor_get(receiver, descriptor);
}
function _class_private_field_init(obj, privateMap, value) {
    _check_private_redeclaration(obj, privateMap);
    privateMap.set(obj, value);
}
function _class_private_field_set(receiver, privateMap, value) {
    var descriptor = _class_extract_field_descriptor(receiver, privateMap, "set");
    _class_apply_descriptor_set(receiver, descriptor, value);
    return value;
}
function _class_private_method_get(receiver, privateSet, fn) {
    if (!privateSet.has(receiver)) throw new TypeError("attempted to get private field on non-instance");
    return fn;
}
function _class_private_method_init(obj, privateSet) {
    _check_private_redeclaration(obj, privateSet);
    privateSet.add(obj);
}
function _define_property(obj, key, value) {
    if (key in obj) Object.defineProperty(obj, key, {
        value: value,
        enumerable: true,
        configurable: true,
        writable: true
    });
    else obj[key] = value;
    return obj;
}
var _pressedKeys = /*#__PURE__*/ new WeakMap(), _client = /*#__PURE__*/ new WeakMap(), _modifierBit = /*#__PURE__*/ new WeakSet(), _keyDescriptionForString = /*#__PURE__*/ new WeakSet();
class CdpKeyboard {
    updateClient(client) {
        _class_private_field_set(this, _client, client);
    }
    async down(key, options = {
        text: void 0,
        commands: []
    }) {
        const description = _class_private_method_get(this, _keyDescriptionForString, keyDescriptionForString).call(this, key);
        const autoRepeat = _class_private_field_get(this, _pressedKeys).has(description.code);
        _class_private_field_get(this, _pressedKeys).add(description.code);
        this._modifiers |= _class_private_method_get(this, _modifierBit, modifierBit).call(this, description.key);
        const text = void 0 === options.text ? description.text : options.text;
        await _class_private_field_get(this, _client).send('Input.dispatchKeyEvent', {
            type: text ? 'keyDown' : 'rawKeyDown',
            modifiers: this._modifiers,
            windowsVirtualKeyCode: description.keyCode,
            code: description.code,
            key: description.key,
            text: text,
            unmodifiedText: text,
            autoRepeat,
            location: description.location,
            isKeypad: 3 === description.location,
            commands: options.commands
        });
    }
    async up(key) {
        const description = _class_private_method_get(this, _keyDescriptionForString, keyDescriptionForString).call(this, key);
        this._modifiers &= ~_class_private_method_get(this, _modifierBit, modifierBit).call(this, description.key);
        _class_private_field_get(this, _pressedKeys).delete(description.code);
        await _class_private_field_get(this, _client).send('Input.dispatchKeyEvent', {
            type: 'keyUp',
            modifiers: this._modifiers,
            key: description.key,
            windowsVirtualKeyCode: description.keyCode,
            code: description.code,
            location: description.location
        });
    }
    async sendCharacter(char) {
        await _class_private_field_get(this, _client).send('Input.insertText', {
            text: char
        });
    }
    charIsKey(char) {
        return !!_keyDefinitions[char];
    }
    async type(text, options = {}) {
        const delay = options.delay || void 0;
        for (const char of text)if (this.charIsKey(char)) await this.press(char, {
            delay
        });
        else {
            if (delay) await new Promise((f)=>setTimeout(f, delay));
            await this.sendCharacter(char);
        }
    }
    async press(key, options = {}) {
        const { delay = null } = options;
        const keys = Array.isArray(key) ? key : [
            key
        ];
        for (const k of keys)await this.down(k, options);
        if (delay) await new Promise((f)=>setTimeout(f, options.delay));
        for (const k of [
            ...keys
        ].reverse())await this.up(k);
    }
    constructor(client){
        _class_private_method_init(this, _modifierBit);
        _class_private_method_init(this, _keyDescriptionForString);
        _class_private_field_init(this, _pressedKeys, {
            writable: true,
            value: new Set()
        });
        _class_private_field_init(this, _client, {
            writable: true,
            value: void 0
        });
        _define_property(this, "_modifiers", 0);
        _class_private_field_set(this, _client, client);
    }
}
function modifierBit(key) {
    if ('Alt' === key) return 1;
    if ('Control' === key) return 2;
    if ('Meta' === key) return 4;
    if ('Shift' === key) return 8;
    return 0;
}
function keyDescriptionForString(keyString) {
    const shift = 8 & this._modifiers;
    const description = {
        key: '',
        keyCode: 0,
        code: '',
        text: '',
        location: 0
    };
    const definition = _keyDefinitions[keyString];
    assert(definition, `Unknown key: "${keyString}"`);
    if (definition.key) description.key = definition.key;
    if (shift && definition.shiftKey) description.key = definition.shiftKey;
    if (definition.keyCode) description.keyCode = definition.keyCode;
    if (shift && definition.shiftKeyCode) description.keyCode = definition.shiftKeyCode;
    if (definition.code) description.code = definition.code;
    if (definition.location) description.location = definition.location;
    if (1 === description.key.length) description.text = description.key;
    if (definition.text) description.text = definition.text;
    if (shift && definition.shiftText) description.text = definition.shiftText;
    if (-9 & this._modifiers) description.text = '';
    return description;
}
export { CdpKeyboard };

//# sourceMappingURL=cdpInput.mjs.map