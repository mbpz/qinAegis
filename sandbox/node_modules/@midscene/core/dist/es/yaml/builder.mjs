import js_yaml from "js-yaml";
function buildYaml(env, tasks) {
    const result = {
        target: env,
        tasks
    };
    return js_yaml.dump(result, {
        indent: 2
    });
}
export { buildYaml };

//# sourceMappingURL=builder.mjs.map