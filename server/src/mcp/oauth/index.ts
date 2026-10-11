/**
 * Signing in to the MCP pool with OAuth, so claude.ai and other connector
 * clients can reach Oya without an API key.
 */
export { oauthRouter, challenge } from './router.ts';
export { connectRouter } from './routes.ts';
