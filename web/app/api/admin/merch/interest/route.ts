import { handleAdminMerchInterest } from "@/lib/merch/interest";

export const dynamic = "force-dynamic";
export async function GET(request: Request) { return handleAdminMerchInterest(request); }
export async function DELETE(request: Request) { return handleAdminMerchInterest(request); }
