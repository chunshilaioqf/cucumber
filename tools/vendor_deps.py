#!/usr/bin/env python3
"""从 Cargo.lock 生成离线 vendor 目录（绕过损坏的镜像 API 跳转端点）。

用法：python3 tools/vendor_deps.py [Cargo.lock] [vendor_dir]
下载源走 rsproxy 对象存储直链（lf3-static，实测稳定高速）。
"""
import concurrent.futures as cf
import hashlib
import json
import os
import shutil
import subprocess
import sys
import tarfile
import urllib.request

LOCK = sys.argv[1] if len(sys.argv) > 1 else "Cargo.lock"
VENDOR = sys.argv[2] if len(sys.argv) > 2 else "vendor"
MIRRORS = ["https://lf3-static.rsproxy.cn/obj/rsproxy/crates",
           "https://lf9-static.rsproxy.cn/obj/rsproxy/crates"]


def lock_packages(path):
    pkgs = []
    cur = {}
    for line in open(path, encoding="utf-8"):
        if line.startswith("[[package]]"):
            if cur.get("name") and cur.get("checksum") and cur.get("version"):
                pkgs.append(cur)
            cur = {}
        elif line.startswith("name = ") and '"' in line:
            cur["name"] = line.split('"')[1]
        elif line.startswith("version = ") and '"' in line and "version" not in cur:
            cur["version"] = line.split('"')[1]
        elif line.startswith('checksum = "'):
            cur["checksum"] = line.split('"')[1]
    if cur.get("name") and cur.get("checksum") and cur.get("version"):
        pkgs.append(cur)
    return pkgs


def fetch(pkg):
    name, ver, want = pkg["name"], pkg["version"], pkg["checksum"]
    dest = os.path.join(VENDOR, f"{name}-{ver}")
    if os.path.exists(os.path.join(dest, ".cargo-checksum.json")):
        return (name, "cached")
    last_err = None
    for mirror in MIRRORS:
        url = f"{mirror}/{name}/{ver}.crate"
        try:
            with urllib.request.urlopen(url, timeout=60) as r:
                blob = r.read()
            got = hashlib.sha256(blob).hexdigest()
            if got != want:
                raise RuntimeError(f"{name} {ver}: sha256 不一致")
            with tarfile.open(fileobj=__import__("io").BytesIO(blob), mode="r:gz") as tf:
                tf.extractall(VENDOR)  # crate 内已有 {name}-{ver}/ 前缀
            # .cargo-checksum.json：files 全量哈希 + package 整体哈希
            files = {}
            for root, _, names in os.walk(dest):
                for n in names:
                    fp = os.path.join(root, n)
                    rel = os.path.relpath(fp, dest).replace(os.sep, "/")
                    if rel == ".cargo-checksum.json":
                        continue
                    files[rel] = hashlib.sha256(open(fp, "rb").read()).hexdigest()
            with open(os.path.join(dest, ".cargo-checksum.json"), "w") as f:
                json.dump({"files": files, "package": want}, f)
            return (name, "ok")
        except Exception as e:  # noqa: BLE001
            last_err = e
    return (name, f"FAIL: {last_err}")


def main():
    os.makedirs(VENDOR, exist_ok=True)
    pkgs = lock_packages(LOCK)
    print(f"依赖 {len(pkgs)} 个")
    fails = []
    with cf.ThreadPoolExecutor(8) as ex:
        for i, (name, status) in enumerate(ex.map(fetch, pkgs), 1):
            print(f"[{i}/{len(pkgs)}] {name}: {status}")
            if status.startswith("FAIL"):
                fails.append(name)
    if fails:
        print("失败：", fails)
        sys.exit(1)
    # 生成 directory source 配置（追加到项目 .cargo/config.toml）
    cfg_dir = os.path.join(os.path.dirname(os.path.abspath(LOCK)), ".cargo")
    os.makedirs(cfg_dir, exist_ok=True)
    cfg = os.path.join(cfg_dir, "config.toml")
    with open(cfg, "w", encoding="utf-8") as f:
        f.write(
            "# 由 tools/vendor_deps.py 生成的离线源（镜像 API 跳转端点故障期间的临时方案）\n"
            "[source.vendored]\n"
            f'directory = "{os.path.abspath(VENDOR)}"\n\n'
            "[source.crates-io]\n"
            "replace-with = \"vendored\"\n"
        )
    print("vendor 完成，已写入", cfg)


if __name__ == "__main__":
    main()
