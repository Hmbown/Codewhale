import { handleMerchInterest } from "@/lib/merch/interest";

export const dynamic = "force-dynamic";
export async function GET(request: Request) { return handleMerchInterest(request); }
export async function POST(request: Request) { return handleMerchInterest(request); }
