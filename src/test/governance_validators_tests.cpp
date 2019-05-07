// Copyright (c) 2014-2018 The Dash Core developers

#include "governance-validators.h"
#include "utilstrencodings.h"

#include "data/proposals_valid.json.h"
#include "data/proposals_invalid.json.h"

#include "test/test_dash.h"

#include <iostream>
#include <fstream>
#include <string>

#include <boost/test/unit_test.hpp>

#include <univalue.h>

extern UniValue read_json(const std::string& jsondata);

BOOST_FIXTURE_TEST_SUITE(governance_validators_tests, BasicTestingSetup)

BOOST_AUTO_TEST_CASE(valid_proposals_test)
{
    // all proposals are valid but expired
    UniValue tests = read_json(std::string(json_tests::proposals_valid, json_tests::proposals_valid + sizeof(json_tests::proposals_valid)));

    BOOST_CHECK_MESSAGE(tests.size(), "Empty `tests`");
    for(size_t i = 0; i < tests.size(); ++i) {
        const UniValue& objProposal = tests[i];

        std::string strHexData = HexStr(objProposal.write());
        CProposalValidator validator(strHexData);
        BOOST_CHECK_MESSAGE(validator.Validate(false), validator.GetErrorMessages());
        BOOST_CHECK_MESSAGE(!validator.Validate(), validator.GetErrorMessages());
    }
}

BOOST_AUTO_TEST_CASE(invalid_proposals_test)
{
    // all proposals are invalid regardless of being expired or not
    // (i.e. we don't even check for expiration here)
    UniValue tests = read_json(std::string(json_tests::proposals_invalid, json_tests::proposals_invalid + sizeof(json_tests::proposals_invalid)));

    BOOST_CHECK_MESSAGE(tests.size(), "Empty `tests`");
    for(size_t i = 0; i < tests.size(); ++i) {
        const UniValue& objProposal = tests[i];

        std::string strHexData = HexStr(objProposal.write());
        CProposalValidator validator(strHexData, false);
        BOOST_CHECK_MESSAGE(!validator.Validate(false), validator.GetErrorMessages());
    }
}

BOOST_AUTO_TEST_SUITE_END()
