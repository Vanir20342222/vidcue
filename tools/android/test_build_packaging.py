"""Offline regression: pruning compiler caches must retain both packaged JNI ABIs."""

import json
import os
import platform
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest

BUILD = Path(__file__).with_name("build.sh")


class BuildPackaging(unittest.TestCase):
    def test_pruning_retains_both_jni_libraries_notices_and_shared_host_cache(self):
        for prune in ("0", "1"):
            with self.subTest(prune=prune), tempfile.TemporaryDirectory() as folder:
                root = Path(folder) / "repo"
                tools = root / "tools/android"
                tools.mkdir(parents=True)
                shutil.copyfile(BUILD, tools / "build.sh")
                (root / "LICENSE").write_text("Synthetic license")
                gradle = root / "apps/android/gradlew"
                gradle.parent.mkdir(parents=True)
                gradle.write_text('#!/bin/sh\ntouch "$PWD/gradle-called"\n')
                gradle.chmod(0o755)
                fake = Path(folder) / "bin"
                fake.mkdir()
                cargo = fake / "cargo"
                cargo.write_text('''#!/usr/bin/env python3
from pathlib import Path
import shutil,sys
args=sys.argv[1:];root=Path.cwd();target=root/'target'
if args[0]=='build':
    triple=args[args.index('--target')+1]
    library=target/triple/'release/libvidcue_mobile.so'
    library.parent.mkdir(parents=True,exist_ok=True)
    library.write_text('synthetic '+triple)
    (target/'release').mkdir(exist_ok=True)
    (target/'release/host-cache').write_text('keep host cache')
elif args[0]=='clean':
    # Reproduce the observed broad cargo-clean behavior, without real build artifacts.
    shutil.rmtree(target)
elif args[0]=='metadata':
    print('{}')
elif args[0]=='run':
    output=Path(args[-1]);output.mkdir(parents=True)
    (output/'inventory.json').write_text('{}')
else:
    raise SystemExit(2)
''')
                cargo.chmod(0o755)
                sdk = Path(folder) / "sdk"
                host = "darwin-x86_64" if platform.system() == "Darwin" else "linux-x86_64"
                ndk_bin = sdk / f"ndk/27.2.12479018/toolchains/llvm/prebuilt/{host}/bin"
                ndk_bin.mkdir(parents=True)
                strip = ndk_bin / "llvm-strip"
                strip.write_text('#!/bin/sh\ncp "$2" "$4"\n')
                strip.chmod(0o755)
                for args in (["init", "-q"], ["config", "user.name", "Fixture"],
                             ["config", "user.email", "fixture@example.invalid"],
                             ["add", "."], ["commit", "-qm", "fixture"]):
                    subprocess.run(["git", *args], cwd=root, check=True)
                env = os.environ.copy()
                env.update(PATH=f"{fake}:/usr/local/bin:/usr/bin:/bin", ANDROID_HOME=str(sdk),
                           YTUI_ANDROID_PRUNE_TARGETS=prune, YTUI_ANDROID_VARIANT="debug",
                           YTUI_ANDROID_ABIS="arm64-v8a x86_64", YTUI_ANDROID_RUST_ONLY="0")
                result = subprocess.run(["bash", str(tools / "build.sh")], cwd=root, env=env,
                                        capture_output=True, text=True)
                self.assertEqual(result.returncode, 0, result.stderr)
                for abi, triple in [("arm64-v8a", "aarch64-linux-android"),
                                    ("x86_64", "x86_64-linux-android")]:
                    self.assertEqual((root/f"target/android-jni/{abi}/libvidcue_mobile.so").read_text(),
                                     "synthetic " + triple)
                    self.assertEqual((root/"target"/triple).exists(), prune == "0")
                self.assertTrue((root/"target/release/host-cache").is_file())
                self.assertEqual(json.loads((root/"target/android-dependencies.json").read_text()), {})
                self.assertTrue((root/"target/android-assets/notices/inventory.json").is_file())
                self.assertTrue((root/"target/android-assets/vidcue-LICENSE").is_file())
                self.assertTrue((root/"gradle-called").is_file())


if __name__ == "__main__":
    unittest.main()
