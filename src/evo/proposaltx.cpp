// Copyright (c) 2019 The Dash Core developers
// Distributed under the MIT software license, see the accompanying
// file COPYING or http://www.opensource.org/licenses/mit-license.php.

#include "proposaltx.h"
#include "deterministicmns.h"
#include "simplifiedmns.h"
#include "specialtx.h"

#include "governance/governance-classes.h"
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

    // Ensure start & end height are superblock heights
    const Consensus::Params& consensusParams = Params().GetConsensus();

    // start height must be a superblock height
    if ((propTx.nStartHeight % consensusParams.nBudgetPaymentsCycleBlocks) != 0) {
        return state.DoS(100, false, REJECT_INVALID, "bad-proptx-start-height");
    }

    // num periods must be greater than 0
    if (propTx.nNumPeriods <= 0) {
        return state.DoS(100, false, REJECT_INVALID, "bad-proptx-num-periods");
    }

    // Validate funding proposal rules
    if (propTx.nProposalType == (uint8_t)PropTxType::Funding) {
        // Ensure amount is positive and under the budget limit for startHeight
        // (all future block heights will have the same or smaller budget)
        CAmount nStartHeightBudget = CSuperblock::GetPaymentsLimit(propTx.nStartHeight);
        if (propTx.nAmount > nStartHeightBudget) {
            return state.DoS(100, false, REJECT_INVALID, "bad-proptx-amount");
        }
    }

    // Validate governance change proposal rules
    if (propTx.nProposalType == (uint8_t)PropTxType::GovChange) {
        // amount == 0
        // script is empty
    }

    // TODO:
    //     * Checks for proptx type (funding vs gov change)
    //     * Ensure payout script is valid
    //     * Validate name / URL

    return true;
}

std::string CProposalTx::ToString() const
{
    return strprintf(
        "CProposalTx(nHeight=%d, nVersion=%d, nStartHeight=%d, nNumPeriods=%d,"
        "nAmount=%lld)",
        nVersion,
        nHeight,
        nStartHeight,
        nNumPeriods,
        nAmount
    );
}
