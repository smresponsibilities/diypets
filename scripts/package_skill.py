import os
import zipfile

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SKILL_SRC = os.path.join(ROOT, "skills", "diypets-creator")
OUT_PATH = os.path.join(ROOT, "dist", "diypets.skill")


def build_skill_archive(out_path=OUT_PATH):
    os.makedirs(os.path.dirname(out_path), exist_ok=True)
    with zipfile.ZipFile(out_path, "w", compression=zipfile.ZIP_DEFLATED) as zf:
        for dirpath, dirnames, filenames in os.walk(SKILL_SRC):
            filenames.sort()
            for i in range(len(filenames)):
                fn = filenames[i]
                full = os.path.join(dirpath, fn)
                arc = os.path.relpath(full, os.path.join(ROOT, "skills")).replace("\\", "/")
                zf.write(full, arc)
    print("Packaged skill archive: " + out_path + " (" + str(os.path.getsize(out_path)) + " bytes)")
    return out_path


if __name__ == "__main__":
    build_skill_archive()
