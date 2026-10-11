/**
 * The console's side of an OAuth sign-in (mounted at /auth/connect): show the
 * signed-in person which app is asking, then allow it for a project they pick or
 * turn it down. Either way the answer is a redirect back to the app.
 */
import { Router } from 'express';
import { userAuthMiddleware } from '../../modules/auth/service.ts';
import { HttpError } from '../../platform/errors.ts';
import { Status } from '../../platform/http-status.ts';
import { allow, deny, describe } from './connect.ts';

/** Routes for a signed-in person answering a connection request. */
export const connectRouter = Router();
/** Every route here needs a signed-in person. */
connectRouter.use(userAuthMiddleware);

/** Someone signed in as another person ("Login as") may not hand that person's project to an app. */
function refuseImpersonation(req) {
  if (req.impersonatedBy) throw new HttpError(Status.FORBIDDEN, 'Not available while signed in as someone else');
}

/** GET /auth/connect/:id, which app is asking. */
connectRouter.get('/:id', async (req, res) => res.json(await describe(req.params.id)));

/** POST /auth/connect/:id/allow, allow it for `project`; answers `{ redirect }`. */
connectRouter.post('/:id/allow', async (req, res) => {
  refuseImpersonation(req);
  res.json(await allow(req.params.id, req.user.id, req.body?.project));
});

/** POST /auth/connect/:id/deny, turn it down; answers `{ redirect }`. */
connectRouter.post('/:id/deny', async (req, res) => res.json(await deny(req.params.id)));

/** Errors become JSON with their status (503 when they have none). */
connectRouter.use((e, req, res, _next) => res.status(e.status || Status.UNAVAILABLE).json({ error: e.message }));
