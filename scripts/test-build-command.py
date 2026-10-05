"""Exercise build_app.command with isolated tool and installation fixtures."""
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

SOURCE = Path(__file__).resolve().parents[1] / 'build_app.command'
APP = '4D Bordro Programı.app'


class BuildCommandTests(unittest.TestCase):
    def run_case(self, case):
        with tempfile.TemporaryDirectory(prefix='bordro-build-test-') as directory:
            root = Path(directory)
            tools = root / 'bin'
            tools.mkdir()
            installed = root / 'Applications' / APP
            installed.mkdir(parents=True)
            (installed / 'version').write_text('installed-old')
            for target in ('src-tauri/target/release', 'target/release',
                           'target/test-host/release'):
                old = root / target / 'bundle/macos' / APP
                old.mkdir(parents=True)
                (old / 'version').write_text('stale')
            if case != 'install-failure':
                (root / 'node_modules').mkdir()
            script = SOURCE.read_text().replace(
                '/Volumes/Lacie/Developer/Rust/cargo/bin', str(tools)
            ).replace('/Applications', str(root / 'Applications'))
            (root / 'build_app.command').write_text(script)
            mocks = {
                'cargo': '''#!/bin/bash
if [ "$1" = --version ]; then echo 'cargo mock'; exit 0; fi
[ "$TEST_CASE" != build-failure ] || exit 7
[ "$TEST_CASE" != missing-output ] || exit 0
mkdir -p "$CARGO_TARGET_DIR/test-host/release/bundle/macos/4D Bordro Programı.app"
printf current > "$CARGO_TARGET_DIR/test-host/release/bundle/macos/4D Bordro Programı.app/version"
''',
                'cargo-tauri': '#!/bin/bash\nexit 0\n',
                'rustc': '#!/bin/bash\necho "host: test-host"\n',
                'bun': '''#!/bin/bash
if [ "$1" = install ] && [ "$TEST_CASE" = install-failure ]; then exit 8; fi
if [ "$2" = wasm:build ] && [ "$TEST_CASE" = wasm-failure ]; then exit 9; fi
exit 0
''',
                'wasm-pack': '#!/bin/bash\nexit 0\n',
                'codesign': '#!/bin/bash\n[ "$TEST_CASE" != signature-failure ]\n',
                'pgrep': '#!/bin/bash\nexit 1\n',
                'clear': '#!/bin/bash\nexit 0\n',
                'xattr': '#!/bin/bash\nexit 0\n',
                'sudo': '#!/bin/bash\nexit 1\n',
                'cp': '#!/bin/bash\n[ "$TEST_CASE" != copy-failure ] || exit 1\nexec /bin/cp "$@"\n',
            }
            for name, body in mocks.items():
                tool = tools / name
                tool.write_text(body)
                tool.chmod(0o755)
            env = {**os.environ, 'HOME': str(root / 'home'),
                   'TEST_CASE': case, 'PATH': f'{tools}:/usr/bin:/bin'}
            result = subprocess.run(['bash', str(root / 'build_app.command')],
                                    input='h\n\n', text=True, capture_output=True,
                                    env=env, timeout=10)
            if case == 'success':
                self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
                self.assertEqual((installed / 'version').read_text(), 'current')
                self.assertEqual((root / 'app' / APP / 'version').read_text(), 'current')
            else:
                self.assertNotEqual(result.returncode, 0, result.stdout + result.stderr)
                self.assertNotIn('Kurulum Başarıyla Tamamlandı', result.stdout)
                self.assertFalse((root / 'app' / APP).exists())
                if case != 'copy-failure':
                    self.assertEqual((installed / 'version').read_text(), 'installed-old')
            if case == 'missing-output':
                self.assertFalse((root / 'target/test-host/release/bundle').exists())

    def test_build_flow(self):
        for case in ('success', 'build-failure', 'missing-output',
                     'install-failure', 'wasm-failure', 'signature-failure',
                     'copy-failure'):
            with self.subTest(case=case):
                self.run_case(case)


if __name__ == '__main__':
    unittest.main()
