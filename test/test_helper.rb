ENV["RAILS_ENV"] = "test"
require File.expand_path(File.dirname(__FILE__) + "/../config/environment")
require 'test/unit'
require 'test_help'
require 'context'
require 'matchy'
require 'action_controller/test_process'
require 'action_controller/integration'
require 'context_on_crack'
require 'rr'
require 'matchy'
require 'pending'
require 'faker'

require "test/blueprints"

class ActiveSupport::TestCase
  include RR::Adapters::RRMethods
  #include AuthenticatedTestHelper

  class << self
    attr_accessor :suite_context
  end

  setup do
    RR.reset
  end

  teardown do
    RR.verify
  end

  def self.transaction(&block)
    ActiveRecord::Base.transaction &block
  end

  def transaction(&block)
    ActiveRecord::Base.transaction &block
  end

  def self.cleanup(*klasses)
    before :all do
      transaction { klasses.each { |k| k.delete_all } }
      ActiveRecord::Base.context_cache = {}
      self.class.suite_context = nil
    end
  end
end

class ActionController::TestCase
  setup do
    RR.reset
  end
  
  def admin_user
    @admin_user ||= User.find_by_email("admin@studioanywhere.digisynd.com") || User.make(:admin)
  end
end

#class Test::Unit::TestCase
#  self.use_transactional_fixtures = true
#  self.use_instantiated_fixtures  = false
#  setup { Sham.reset }
#end
#