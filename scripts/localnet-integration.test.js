import { SigningCosmWasmClient }   from "@cosmjs/cosmwasm-stargate";
import { DirectSecp256k1HdWallet, coin } from "@cosmjs/proto-signing";
import { calculateFee, GasPrice }  from "@cosmjs/stargate";
import * as fs from "fs";

const SG_FILE_PATH = `${__dirname}/../artifacts/shitcoin-garden.wasm`;
const MD_FILE_PATH = `${__dirname}/../artifacts/mock-dex.wasm`;

const walletMnemonic = process.env.DEVNET_WALLET_MNEMONIC;
const walletAddress  = process.env.DEVNET_WALLET_ADDRESS;
const chainPrefix    = process.env.DEVNET_CHAIN_PREFIX;
const chainDenom     = process.env.DEVNET_CHAIN_DENOM;
const chainRpc       = process.env.DEVNET_CHAIN_RPC;
const chainGasPrice  = process.env.DEVNET_CHAIN_GAS_PRICE;

if (!walletMnemonic) throw new Error("DEVNET_WALLET_MNEMONIC env var not set");
if (!walletAddress)  throw new Error("DEVNET_WALLET_ADDRESS env var not set");
if (!chainPrefix)    throw new Error("DEVNET_CHAIN_PREFIX env var not set");
if (!chainDenom)     throw new Error("DEVNET_CHAIN_DENOM env var not set");
if (!chainRpc)       throw new Error("DEVNET_CHAIN_RPC env var not set");
if (!chainGasPrice)  throw new Error("DEVNET_CHAIN_GAS_PRICE env var not set");

const gasPrice = GasPrice.fromString(chainGasPrice);
const wallet = await DirectSecp256k1HdWallet.fromMnemonic(walletMnemonic, { prefix: chainPrefix });
const client = await SigningCosmWasmClient.connectWithSigner(chainRpc, wallet);

const uploadFee = calculateFee(10_500_000, gasPrice);

const sgWasm = fs.readFileSync(SG_FILE_PATH);
const sgUploadReceipt = await client.upload(walletAddress, sgWasm, uploadFee, "SG-IYKYK");

const mdWasm = fs.readFileSync(MD_FILE_PATH);
const mdUploadReceipt = await client.upload(walletAddress, mdWasm, uploadFee, "MD-IYKYK");

console.log("uploaded contracts")

const instantiateFee = calculateFee(500_000, gasPrice);

const { contractAddress: mdContractAddress} = await client.instantiate(
  walletAddress,
  mdUploadReceipt.codeId,
  {},
  "MD-IYKYK",
  instantiateFee,
);

console.log(`instantiated mock dex: ${mdContractAddress}`);

const sgInitMsg = {
    pool_factory_address: mdContractAddress,
    fee_recipient: walletAddress,
    create_fee_denom: chainDenom,
    create_fee: "10000",
    presale_denom: chainDenom,
    presale_length: 20, // seconds
    presale_fee_rate: 50,
};
const { contractAddress: sgContractAddress } = await client.instantiate(
  walletAddress,
  sgUploadReceipt.codeId,
  sgInitMsg,
  "SG-IYKYK",
  instantiateFee,
);

console.log(`instantiated shitcoin garden: ${sgContractAddress}`);

const executeFee = calculateFee(500_000, gasPrice);

const createShitcoinMsg = {
  create_shitcoin: {
    ticker: "LNL",
    name: "localnet larper",
    supply: "1",
  }
};

await client.execute(walletAddress, sgContractAddress, createShitcoinMsg, executeFee, "", [coin(10_000, chainDenom)]);

let denom = "";

{
  const shitcoins = await client.queryContractSmart(sgContractAddress, { shitcoins: {} });

  const shitcoin = shitcoins.shitcoins[0];

  console.log(`created shitcoin: ${shitcoin.denom}`);

  denom = shitcoin.denom;
}

const enterPresaleMsg = {
  enter_presale: { denom }
};

await client.execute(walletAddress, sgContractAddress, enterPresaleMsg, executeFee, "", [coin(100_000, chainDenom)]);

{
  const shitcoin = await client.queryContractSmart(sgContractAddress, { shitcoin_metadata: { denom } });

  console.log(`shitcoin presale entered: ${shitcoin.presale_raise}`);
}

console.log("waiting 20 seconds for presale to have ended");

await Bun.sleep(20_000);

const launchShitcoinMsg = {
  launch_shitcoin: { denom }
};

await client.execute(walletAddress, sgContractAddress, launchShitcoinMsg, executeFee);

{
  const shitcoin = await client.queryContractSmart(sgContractAddress, { shitcoin_metadata: { denom } });

  console.log(`shitcoin launched: ${shitcoin.launched}`);
}

const claimShitcoinMsg = {
  claim_shitcoin: { denom }
};

await client.execute(walletAddress, sgContractAddress, claimShitcoinMsg, executeFee);

const shitcoinBalance = await client.getBalance(walletAddress, denom);

console.log(`claimed shitcoin: ${shitcoinBalance.amount}`);

