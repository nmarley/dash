// Copyright (c) 2019 The Dash Core developers
// Distributed under the MIT software license, see the accompanying
// file COPYING or http://www.opensource.org/licenses/mit-license.php.

#include "proposaltx.h"
#include "deterministicmns.h"
#include "simplifiedmns.h"
#include "specialtx.h"

#include "chainparams.h"
#include "consensus/merkle.h"
#include "univalue.h"
#include "validation.h"

bool CheckProposalTx(const CTransaction& tx, const CBlockIndex* pindexPrev, CValidationState& state)
{
    if (tx.nType != TRANSACTION_PROPOSAL) {
        return state.DoS(100, false, REJECT_INVALID, "bad-proptx-type");
    }

    CProposalTx propTx;
    if (!GetTxPayload(tx, propTx)) {
        return state.DoS(100, false, REJECT_INVALID, "bad-proptx-payload");
    }

    if (propTx.nVersion == 0 || propTx.nVersion > CProposalTx::CURRENT_VERSION) {
        return state.DoS(100, false, REJECT_INVALID, "bad-proptx-version");
    }

    if (pindexPrev && pindexPrev->nHeight + 1 != propTx.nHeight) {
        return state.DoS(100, false, REJECT_INVALID, "bad-proptx-height");
    }

    return true;
}

std::string CProposalTx::ToString() const
{
    return strprintf("CProposalTx(nHeight=%d, nVersion=%d, TODO/finish...)",
        nVersion, nHeight);
}

void CProposalTx::ToJson(UniValue& obj) const
{
    obj.clear();
    obj.setObject();
    obj.push_back(Pair("version", (int)nVersion));
    obj.push_back(Pair("height", (int)nHeight));
    // TODO: Finish...
}
