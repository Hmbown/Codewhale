import recipes from "./recipes.generated.json";

const REPOSITORY = "https://github.com/Hmbown/codewhale-ratatui";

/** Exact public functions exported from the library's compiled recipes example. */
export function getRecipe(id?: string): { code: string; line: number } | null {
  if (!id || !Object.prototype.hasOwnProperty.call(recipes.recipes, id)) return null;
  return recipes.recipes[id as keyof typeof recipes.recipes];
}

export function recipeSourceUrl(line: number) {
  return `${REPOSITORY}/blob/${recipes.sourceRevision}/examples/recipes.rs#L${line}`;
}

export function learningGuideUrl(anchor?: string) {
  return `${REPOSITORY}/blob/${recipes.sourceRevision}/GETTING-STARTED.md${anchor ? `#${anchor}` : ""}`;
}

export const STARTER_SOURCE = `${REPOSITORY}/blob/${recipes.sourceRevision}/examples/starter.rs`;
export const RECIPE_SOURCE = `${REPOSITORY}/blob/${recipes.sourceRevision}/examples/recipes.rs`;
export const INSTALL = `[dependencies]
codewhale-ratatui = { git = "${REPOSITORY}", rev = "${recipes.sourceRevision}" }
ratatui = { version = "0.30.2", default-features = false, features = ["std", "crossterm_0_29"] }
crossterm = "0.29"`;

export const TRY_STARTER = `git clone ${REPOSITORY}
cd codewhale-ratatui
cargo run --locked --example starter`;
