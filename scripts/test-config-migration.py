#!/usr/bin/env python3
"""Isolated HTTP migration: explicit source/secrets, preview CAS and recovery.

Uses only seeded configuration files, never the user's configuration or models.
"""
import importlib.util
import json
from pathlib import Path
import sys
import tempfile

loader = importlib.util.spec_from_file_location('local_web_smoke', Path(__file__).with_name('test-local-web.py'))
module = importlib.util.module_from_spec(loader)
loader.loader.exec_module(module)
require = module.require


def main():
    with tempfile.TemporaryDirectory(prefix='qinaegis-config-migration-') as directory:
        smoke = module.Smoke(Path(sys.argv[1]).resolve(), Path(directory).resolve())
        try:
            smoke.cli('start', '--no-open', '--port', str(smoke.port))
            smoke.instance = smoke.read_instance()
            smoke.authenticate()
            project = 'migration-fixture'
            smoke.rpc('createProject', {'name': project, 'url': 'http://127.0.0.1:9', 'tech_stack': []})
            target_key, source_key = 'migration-target-secret', 'migration-source-secret'
            smoke.secrets.extend([target_key, source_key])
            smoke.rpc('setConfig', {'expected_revision': 0, 'config': {'llm': {'api_key': target_key, 'model': 'target-model'}}})
            target = smoke.data / 'config.toml'
            original = target.read_bytes()
            source = smoke.data / 'projects' / project / 'qinAegis.toml'
            source.write_text(original.decode().replace('target-model', 'source-model').replace(target_key, source_key))
            original_source = source.read_bytes()
            params = {'source_id': 'project_override', 'project': project}
            sources = smoke.rpc('listConfigMigrationSources', {'project': project})
            require(any(item['id'] == 'project_override' and item['status'] == 'ready' for item in sources['sources']), 'managed source unavailable')
            smoke.rpc('listConfigMigrationSources', {'path': str(source)}, success=False)
            smoke.rpc('previewConfigMigration', {'source_id': str(source), 'project': project}, success=False)
            preview = smoke.rpc('previewConfigMigration', params)
            require(preview['target_revision'] == 1 and preview['source_summary']['llm']['model'] == 'source-model', 'migration summary incorrect')
            require('llm.model' in preview['choices']['keep_target']['changed_fields'], 'migration diff omitted changed model')
            apply = {'preview_token': preview['preview_token'], 'secret_policy': 'keep_target', 'approved': True}
            smoke.rpc('applyConfigMigration', {'preview_token': preview['preview_token'], 'approved': True}, success=False)
            smoke.rpc('applyConfigMigration', {**apply, 'approved': False}, success=False)
            require(target.read_bytes() == original, 'unapproved migration changed target')
            # A preview binds source bytes, even if its legacy revision is unchanged.
            source.write_bytes(original_source + b'\n# concurrent source edit\n')
            smoke.rpc('applyConfigMigration', apply, success=False)
            require(target.read_bytes() == original, 'stale source overwrote target')
            source.write_bytes(original_source)
            preview = smoke.rpc('previewConfigMigration', params)
            apply['preview_token'] = preview['preview_token']
            result = smoke.rpc('applyConfigMigration', apply)
            require(result['revision'] == 2 and not result['unchanged'], 'migration did not advance revision')
            require(target_key.encode() in target.read_bytes() and source_key.encode() not in target.read_bytes(), 'keep-target policy changed credentials')
            require((smoke.data / 'config.toml.backup').read_bytes() == original, 'migration lost previous target backup')
            require(source.read_bytes() == original_source, 'migration changed source file')
            migrated = target.read_bytes()
            repeated = smoke.rpc('applyConfigMigration', apply)
            require(repeated['unchanged'] and repeated['revision'] == 2 and target.read_bytes() == migrated, 'repeat migration is not idempotent')
            require(smoke.rpc('getState')['config']['llm']['model'] == 'source-model', 'service did not reload migrated configuration')
            preview = smoke.rpc('previewConfigMigration', params)
            smoke.rpc('applyConfigMigration', {'preview_token': preview['preview_token'], 'secret_policy': 'use_source', 'approved': True})
            require(source_key.encode() in target.read_bytes() and target_key.encode() not in target.read_bytes(), 'explicit source-secret choice ignored')
            state = smoke.rpc('getState')
            preview = smoke.rpc('previewConfigMigration', params)
            smoke.rpc('setConfig', {'expected_revision': state['config_revision'], 'config': {'llm': {'model': 'newer-target'}}})
            concurrent = target.read_bytes()
            smoke.rpc('applyConfigMigration', {'preview_token': preview['preview_token'], 'secret_policy': 'keep_target', 'approved': True}, success=False)
            require(target.read_bytes() == concurrent, 'migration overwrote newer target')
            print('PASS config migration: known sources, secret redaction/choice, exact-byte preview conflicts, backup and idempotence', flush=True)
        finally:
            smoke.cleanup()


if __name__ == '__main__':
    main()
