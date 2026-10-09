export const inject = ['avatars']
export function apply(ctx) {
  ctx.avatars.registerPack({ path: 'avatars/avatar.json' })
}
