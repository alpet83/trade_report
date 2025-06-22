# /sandwich_pack.py
# Modified: 2025-06-21 18:30:00 EEST

import os
import datetime
import pytz
import hashlib
import json
import re
from pathlib import Path

def get_file_mod_time(file_path):
    """Получает время модификации файла в формате YYYY-MM-DD HH:MM:SS EEST."""
    mtime = os.path.getmtime(file_path)
    eest = pytz.timezone("Europe/Tallinn")  # EEST
    mod_time = datetime.datetime.fromtimestamp(mtime, tz=eest)
    return mod_time.strftime("%Y-%m-%d %H:%M:%S %Z")

def is_hidden_file(filepath):
    """Проверяет, является ли файл скрытым (начинается с точки)."""
    return any(part.startswith(".") for part in filepath.parts)

def estimate_tokens(content):
    """Оценивает количество токенов (примерно 1 токен = 4 символа)."""
    return len(content) // 4

def compute_md5(content):
    """Вычисляет MD5-хэш содержимого."""
    return hashlib.md5(content.encode("utf-8")).hexdigest()

def extract_entities(content, extension):
    """Извлекает структуры, трейты и функции из .rs файлов."""
    if extension != ".rs":
        return []
    entities = []
    # Структуры (struct ...)
    struct_pattern = re.compile(r"(?:pub\s+)?struct\s+(\w+)\s*{")
    for match in struct_pattern.finditer(content):
        entities.append({"type": "struct", "name": match.group(1)})
    # Трейты (trait ...)
    trait_pattern = re.compile(r"(?:pub\s+)?trait\s+(\w+)\s*{")
    for match in trait_pattern.finditer(content):
        entities.append({"type": "trait", "name": match.group(1)})
    # Функции (fn ...)
    fn_pattern = re.compile(r"(?:pub\s+)?(?:async\s+)?fn\s+(\w+)\s*\(")
    for match in fn_pattern.finditer(content):
        entities.append({"type": "function", "name": match.group(1)})
    return entities

def extract_dependencies(content, extension, src, all_files):
    """Извлекает зависимости из файла (.rs)."""
    if extension != ".rs":
        return []
    dependencies = []
    # Модули (pub mod ...)
    module_pattern = re.compile(r"pub\s+mod\s+(\w+);")
    for match in module_pattern.finditer(content):
        module_name = match.group(1)
        module_path = f"{os.path.dirname(src)}/{module_name}/mod.rs".replace("\\", "/")
        if not module_path.startswith("/"):
            module_path = f"/{module_path}"
        if module_path not in all_files:
            module_path = f"{os.path.dirname(src)}/{module_name}.rs".replace("\\", "/")
            if not module_path.startswith("/"):
                module_path = f"/{module_path}"
        if module_path in all_files:
            dependencies.append({"type": "module", "details": module_name, "src": module_path})
        else:
            dependencies.append({"type": "module", "details": module_name})
    # Импорты (use crate::...)
    import_pattern = re.compile(r"use\s+crate::([\w:]+)(?:\{([\w,: ]+)\})?;")
    for match in import_pattern.finditer(content):
        path = match.group(1).replace("::", "/")
        if match.group(2):
            for item in match.group(2).split(","):
                item = item.strip()
                for file_path in all_files:
                    if item in all_files[file_path] and file_path.endswith(".rs"):
                        dependencies.append({"type": "import", "details": item, "src": file_path})
                        break
                else:
                    dependencies.append({"type": "import", "details": item})
        else:
            import_path = f"/src/{path}.rs".replace("\\", "/")
            if import_path in all_files:
                dependencies.append({"type": "import", "details": path.split("/")[-1], "src": import_path})
            else:
                dependencies.append({"type": "import", "details": path.split("/")[-1]})
    # Вызовы функций (примерная эвристика)
    call_pattern = re.compile(r"\b(\w+)\s*\(")
    for match in call_pattern.finditer(content):
        func_name = match.group(1)
        for file_path, file_data in all_files.items():
            if file_path.endswith(".rs"):
                if func_name in file_data.get("entities", []) and any(e["name"] == func_name and e["type"] == "function" for e in file_data["entities"]):
                    dependencies.append({"type": "call", "details": func_name, "src": file_path})
    return dependencies

def collect_files(root_dir):
    """Собирает все файлы в каталоге, исключая скрытые."""
    files_content = {}
    all_files = {}
    root_path = Path(root_dir).parent
    for file_path in Path(root_dir).rglob("*"):
        if file_path.is_file() and not is_hidden_file(file_path):
            relative_path = f"/{file_path.relative_to(root_path)}".replace("\\", "/")
            with open(file_path, "r", encoding="utf-8") as f:
                try:
                    content = f.read()
                except UnicodeDecodeError as e:
                    print(f"Skipping {file_path}: Cannot decode as UTF-8 ({e})")
                    continue
            mod_time = get_file_mod_time(file_path)
            entities = extract_entities(content, Path(file_path).suffix.lower())
            files_content[relative_path] = {
                "mod_time": mod_time,
                "content": content,
                "extension": Path(file_path).suffix.lower(),
                "entities": entities
            }
            all_files[relative_path] = {"content": content, "entities": entities}
    for file_path in root_path.glob("*.toml"):
        if not is_hidden_file(file_path):
            relative_path = f"/{file_path.name}".replace("\\", "/")
            with open(file_path, "r", encoding="utf-8") as f:
                try:
                    content = f.read()
                except UnicodeDecodeError as e:
                    print(f"Skipping {file_path}: Cannot decode as UTF-8 ({e})")
                    continue
            mod_time = get_file_mod_time(file_path)
            files_content[relative_path] = {
                "mod_time": mod_time,
                "content": content,
                "extension": Path(file_path).suffix.lower(),
                "entities": []
            }
            all_files[relative_path] = {"content": content, "entities": []}
    return files_content, all_files

def write_sandwich_files(files_content, all_files, output_dir, max_size=80_000):
    """Записывает файлы в 'сэндвичи' и единый индекс."""
    os.makedirs(output_dir, exist_ok=True)
    current_size = 0
    current_tokens = 0
    current_file_index = 1
    current_content = []
    current_index = []
    global_index = {
        "project_version": "0.1.0",
        "context_date": get_file_mod_time(output_dir),
        "sandwiches": []
    }
    current_line = 1
    output_file = Path(output_dir) / f"sandwich_{current_file_index}.txt"

    for src, file in sorted(files_content.items()):  # Сортировка для стабильного порядка
        tag = "file" if file["extension"] not in [".rs", ".md", ".toml"] else \
              "rustc" if file["extension"] == ".rs" else \
              "markdown_code" if file["extension"] == ".md" else "config_toml"
        block = f'<{tag} src="{src}" mod_time="{file["mod_time"]}">\n{file["content"]}\n</{tag}>\n'
        block_size = len(block.encode("utf-8"))
        block_tokens = estimate_tokens(block)
        block_lines = block.count("\n") + 1
        block_md5 = compute_md5(block)

        if current_size + block_size > max_size or current_tokens + block_tokens > 20_000:
            with open(output_file, "w", encoding="utf-8") as f:
                f.write("".join(current_content))
            print(f"Created {output_file} ({current_size} bytes, ~{current_tokens} tokens)")
            global_index["sandwiches"].append({
                "file": f"sandwich_{current_file_index}.txt",
                "files": current_index
            })
            current_file_index += 1
            output_file = Path(output_dir) / f"sandwich_{current_file_index}.txt"
            current_content = []
            current_index = []
            current_size = 0
            current_tokens = 0
            current_line = 1

        current_content.append(block)
        current_index.append({
            "src": src,
            "start_line": current_line,
            "tokens": block_tokens,
            "mod_time": file["mod_time"],
            "md5": block_md5,
            "dependencies": extract_dependencies(file["content"], file["extension"], src, all_files),
            "entities": file["entities"]
        })
        current_size += block_size
        current_tokens += block_tokens
        current_line += block_lines

    if current_content:
        with open(output_file, "w", encoding="utf-8") as f:
            f.write("".join(current_content))
        print(f"Created {output_file} ({current_size} bytes, ~{current_tokens} tokens)")
        global_index["sandwiches"].append({
            "file": f"sandwich_{current_file_index}.txt",
            "files": current_index
        })

    # Записываем единый индекс
    global_index_file = Path(output_dir) / "sandwiches_index.json"
    with open(global_index_file, "w", encoding="utf-8") as f:
        json.dump(global_index, f, indent=2)
    print(f"Created {global_index_file}")

def main():
    project_dir = "./src"
    output_dir = "./sandwiches"
    files_content, all_files = collect_files(project_dir)
    print(f"Collected {len(files_content)} files")
    write_sandwich_files(files_content, all_files, output_dir)

if __name__ == "__main__":
    main()