const fs = require("fs");
const path = require("path");

const localesDir = path.join(__dirname, "..", "renderer", "locales");
const deadKeys = [
  ["HEADER", "GITHUB_BUTTON_TITLE"],
  ["FOOTER", "TITLE"],
  ["FOOTER", "LINK_TITLE"],
  ["FOOTER", "NEWS_TITLE"],
  ["SETTINGS", "DONATE", "DESCRIPTION"],
  ["SETTINGS", "DONATE", "BUTTON_TITLE"],
  ["SETTINGS", "SUPPORT", "TITLE"],
  ["SETTINGS", "SUPPORT", "DOCS_BUTTON_TITLE"],
  ["SETTINGS", "SUPPORT", "EMAIL_BUTTON_TITLE"],
  ["SETTINGS", "CUSTOM_MODELS", "LINK_TITLE"],
  ["ERRORS", "OPEN_DOCS_TITLE"],
  ["ERRORS", "OPEN_DOCS_BUTTON_TITLE"],
];

const deletePath = (obj, keys) => {
  if (!obj || keys.length === 0) return;
  const [head, ...rest] = keys;
  if (rest.length === 0) {
    delete obj[head];
  } else if (obj[head] && typeof obj[head] === "object") {
    deletePath(obj[head], rest);
  }
};

const removeEmptyObjects = (obj) => {
  for (const key of Object.keys(obj)) {
    if (obj[key] && typeof obj[key] === "object" && !Array.isArray(obj[key])) {
      removeEmptyObjects(obj[key]);
      if (Object.keys(obj[key]).length === 0) {
        delete obj[key];
      }
    }
  }
};

const scrubGithub = (text) => {
  const idx = text.toLowerCase().indexOf("git");
  if (idx === -1) return text;
  const before = text.slice(0, idx).trim();
  return before.replace(/[.,;\s]+$/, "").trim();
};

for (const file of fs.readdirSync(localesDir)) {
  if (!file.endsWith(".json")) continue;
  const filePath = path.join(localesDir, file);
  const data = JSON.parse(fs.readFileSync(filePath, "utf-8"));
  for (const keys of deadKeys) {
    deletePath(data, keys);
  }
  if (data.ERRORS?.EXCEPTION_ERROR?.DESCRIPTION) {
    data.ERRORS.EXCEPTION_ERROR.DESCRIPTION = scrubGithub(
      data.ERRORS.EXCEPTION_ERROR.DESCRIPTION,
    );
  }
  removeEmptyObjects(data);
  fs.writeFileSync(filePath, JSON.stringify(data, null, 2) + "\n");
}

console.log("done");
