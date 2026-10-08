import { BodyReadError, readBoundedBody } from "./bounded-body";

export class FormBodyError extends Error {
  constructor(
    readonly status: 400 | 413 | 415,
    message: string
  ) {
    super(message);
    this.name = "FormBodyError";
  }
}

export async function readBoundedUrlEncodedForm(
  request: Request,
  maxBytes: number
): Promise<URLSearchParams> {
  const mediaType = request.headers.get("content-type")?.split(";", 1)[0]?.trim().toLowerCase();
  if (mediaType !== "application/x-www-form-urlencoded") {
    throw new FormBodyError(415, "expected application/x-www-form-urlencoded");
  }

  try {
    const bytes = await readBoundedBody(request, maxBytes);
    return new URLSearchParams(new TextDecoder().decode(bytes));
  } catch (cause) {
    if (cause instanceof BodyReadError) throw new FormBodyError(cause.status, cause.message);
    throw cause;
  }
}
